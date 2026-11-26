use axutils::config::{ConfigError, ConfigFormat, ConfigLoader, ConfigValue};

#[cfg(feature = "jwt")]
mod jwt {
    use axutils::jwt::{
        JwtAlgorithm, JwtCodec, JwtConfig, JwtSigningKey, JwtValidation, JwtVerificationKey,
    };

    fn jwt_codec() -> JwtCodec {
        let validation = JwtValidation::new().with_leeway(0).unwrap();
        JwtCodec::new(
            JwtConfig::new(
                JwtAlgorithm::Hs256,
                Some(JwtSigningKey::from_hmac_secret([0x11; 32]).unwrap()),
                Some(JwtVerificationKey::from_hmac_secret([0x11; 32]).unwrap()),
                validation,
            )
            .unwrap(),
        )
    }

    #[test]
    fn jwt_provider_round_trip() {
        // 独立计算的 HS256 向量，未选择 provider 时也有合法 token 可用于验签。
        const TOKEN: &str = concat!(
            "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.",
            "eyJleHAiOjQxMDI0NDQ4MDB9.",
            "_V9XHM5ZnESSHsyE-IV4_XbvvXoLtdmRFOY5FVCDLKY"
        );
        let codec = jwt_codec();
        let claims = serde_json::json!({"exp": 4_102_444_800u64});

        #[cfg(feature = "jwt-dual-provider")]
        {
            use axutils::jwt::JwtError;
            use jsonwebtoken::crypto::aws_lc;

            let expected = JwtError::InvalidConfig {
                field: "crypto_provider",
            };
            assert_eq!(codec.encode(&claims).unwrap_err(), expected);
            assert_eq!(
                codec.decode::<serde_json::Value>(TOKEN).unwrap_err(),
                expected
            );
            aws_lc::DEFAULT_PROVIDER.install_default().unwrap();
        }

        assert_eq!(codec.decode::<serde_json::Value>(TOKEN).unwrap(), claims);
        let token = codec.encode(&claims).unwrap();
        assert_eq!(codec.decode::<serde_json::Value>(&token).unwrap(), claims);
        check_ec_verification_key_errors();
    }

    fn check_ec_verification_key_errors() {
        use axutils::jwt::JwtError;

        // 用仓库的 ec256 测试密钥独立签发。相同向量在两个 provider 下走公开 decode 入口。
        const TOKEN: &str = concat!(
            "eyJ0eXAiOiJKV1QiLCJhbGciOiJFUzI1NiJ9.eyJleHAiOjQxMDI0NDQ4MDB9.",
            "xLH_csRZfbSp8qMotG1nl-rBJCuiWugoRuZ5z_lyxKf7-JhPLpFPCKOmkGzHLMhRblnIbHdV_XBdxtT2Odx_YQ"
        );
        const PUBLIC_POINT: &[u8] = &[
            4, 195, 178, 64, 161, 79, 224, 37, 182, 73, 189, 95, 179, 8, 235, 212, 247, 33, 73,
            171, 65, 77, 11, 247, 157, 8, 196, 76, 239, 195, 252, 121, 4, 193, 8, 53, 19, 43, 92,
            176, 73, 134, 172, 206, 244, 25, 190, 119, 162, 91, 168, 13, 181, 97, 9, 157, 212, 171,
            120, 71, 50, 201, 71, 85, 190,
        ];
        let codec_for = |point: &[u8]| {
            let key = JwtVerificationKey::from_ec_der(point).unwrap();
            JwtCodec::new(
                JwtConfig::new(JwtAlgorithm::Es256, None, Some(key), JwtValidation::new()).unwrap(),
            )
        };
        let codec = codec_for(PUBLIC_POINT);
        assert_eq!(
            codec.decode::<serde_json::Value>(TOKEN).unwrap(),
            serde_json::json!({"exp": 4_102_444_800u64})
        );
        // 保留原签名，改用另一个合法的 exp 值。
        let mismatched = TOKEN.replace("eyJleHAiOjQxMDI0NDQ4MDB9", "eyJleHAiOjQxMDI0NDQ4MDF9");
        assert_eq!(
            codec.decode::<serde_json::Value>(&mismatched).unwrap_err(),
            JwtError::InvalidToken { segment: "token" }
        );

        let mut off_curve = [0; 65];
        off_curve[0] = 4;
        for point in [&PUBLIC_POINT[..64], &off_curve] {
            assert_eq!(
                codec_for(point)
                    .decode::<serde_json::Value>(TOKEN)
                    .unwrap_err(),
                JwtError::UnsupportedKeyFormat {
                    kind: "verification_key"
                }
            );
        }
    }

    #[cfg(not(feature = "jwt-dual-provider"))]
    #[test]
    fn jwt_round_trips_standard_dates_and_typed_business_fields() {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, PartialEq, Deserialize, Serialize)]
        struct Measurements {
            #[serde(rename = "$serde_json::private::Number")]
            label: String,
            signed: [i128; 3],
            unsigned: [u128; 3],
            fraction: f64,
        }

        #[derive(Debug, PartialEq, Deserialize, Serialize)]
        struct Claims {
            exp: u64,
            nbf: u64,
            measurements: Measurements,
        }

        let claims = Claims {
            exp: u64::MAX,
            nbf: 0,
            measurements: Measurements {
                label: "sample".into(),
                signed: [i128::MIN, i128::from(i64::MIN) - 1, i128::MAX],
                unsigned: [u128::from(u64::MAX), u128::from(u64::MAX) + 1, u128::MAX],
                fraction: 1.5,
            },
        };
        let codec = jwt_codec();
        let token = codec.encode(&claims).unwrap();
        assert_eq!(codec.decode::<Claims>(&token).unwrap(), claims);
    }

    #[cfg(not(feature = "jwt-dual-provider"))]
    #[test]
    fn jwt_counts_only_containers_at_the_depth_limit() {
        use serde_json::value::RawValue;

        let codec = jwt_codec();
        let label = serde_json::to_string(r#"["{\"#).unwrap();
        let leaves = format!("1.25e-3,{},{},{label}", i128::MIN, u128::MAX);
        let text = format!(
            r#"{{"exp":{},"samples":{}{}{} }}"#,
            u64::MAX,
            "[".repeat(31),
            leaves,
            "]".repeat(31)
        );
        let claims: &RawValue = serde_json::from_str(&text).unwrap();
        let token = codec.encode(&claims).unwrap();
        assert_eq!(codec.decode::<Box<RawValue>>(&token).unwrap().get(), text);
    }
}

#[test]
fn numbers_remain_scalars_at_the_depth_limit() {
    let loader = ConfigLoader::new().with_max_depth(1).unwrap();
    assert_eq!(
        loader.parse_value("1.5", ConfigFormat::Json),
        Ok(ConfigValue::Float(1.5))
    );
    let list = loader
        .parse_value(
            "[1.5,2e1,-9223372036854775808,9223372036854775807]",
            ConfigFormat::Json,
        )
        .unwrap();
    assert_eq!(
        list.as_array().unwrap(),
        [
            ConfigValue::Float(1.5),
            ConfigValue::Float(20.0),
            ConfigValue::Integer(i64::MIN),
            ConfigValue::Integer(i64::MAX)
        ]
    );
    assert_eq!(
        loader.parse_value("[[]]", ConfigFormat::Json),
        Err(ConfigError::DepthLimitExceeded { limit: 1 })
    );
}

#[test]
fn marker_keys_are_ordinary_decoded_object_keys() {
    let loader = ConfigLoader::new();
    for key in [
        "$serde_json::private::Number",
        "$serde_json::private::RawValue",
    ] {
        let text = format!(r#"{{"{key}":"1.5"}}"#);
        let value = loader.parse_value(&text, ConfigFormat::Json).unwrap();
        assert_eq!(value.as_table().unwrap()[key].as_str(), Some("1.5"));
        let text = format!(r#"{{"{key}":[1.5],"other":true}}"#);
        let value = loader.parse_value(&text, ConfigFormat::Json).unwrap();
        assert_eq!(
            value.as_table().unwrap()[key].as_array().unwrap(),
            [ConfigValue::Float(1.5)]
        );
        assert_eq!(value.get("other"), Some(&ConfigValue::Bool(true)));
        let text = format!(r#"[{{"{key}":1,"\u0024{}":2}}]"#, &key[1..]);
        assert_eq!(
            loader.parse_value(&text, ConfigFormat::Json),
            Err(ConfigError::DuplicateKey { key: key.into() })
        );
    }
}

#[test]
fn range_errors_report_the_enclosing_field_through_arrays() {
    let loader = ConfigLoader::new();
    for number in [
        "9223372036854775808",
        "18446744073709551616",
        "-9223372036854775809",
    ] {
        let text = format!(r#"{{"outer":[{{"items":[[0,{number}]]}}]}}"#);
        assert_eq!(
            loader.parse_value(&text, ConfigFormat::Json),
            Err(ConfigError::ValueOutOfRange {
                key: "items".into()
            })
        );
    }
}
