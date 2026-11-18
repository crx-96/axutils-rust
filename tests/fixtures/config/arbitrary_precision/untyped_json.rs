use axutils::config::{ConfigError, ConfigFormat, ConfigLoader, ConfigValue};

#[cfg(feature = "jwt")]
mod jwt {
    use axutils::jwt::{
        JwtAlgorithm, JwtCodec, JwtConfig, JwtSigningKey, JwtValidation, JwtVerificationKey,
    };
    use serde::{Deserialize, Serialize};
    use serde_json::value::RawValue;

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
    fn jwt_round_trips_standard_dates_and_typed_business_fields() {
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

    #[test]
    fn jwt_counts_only_containers_at_the_depth_limit() {
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
