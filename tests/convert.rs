#[cfg(any(feature = "itoa", feature = "ryu", feature = "zmij", feature = "uuid"))]
use axutils::ConvertUtils;

#[test]
fn convert_utils_is_available_through_all_public_paths() {
    let _: axutils::ConvertUtils = axutils::ConvertUtils;
    let _: axutils::convert::ConvertUtils = axutils::convert::ConvertUtils;
    let _: axutils::utils::ConvertUtils = axutils::utils::ConvertUtils;
    let _: axutils::utils::convert_utils::ConvertUtils =
        axutils::utils::convert_utils::ConvertUtils;
}

#[cfg(feature = "itoa")]
#[test]
fn integer_conversion_covers_root_and_convert_paths() {
    use axutils::{IntegerBuffer, IntegerValue};

    macro_rules! assert_round_trip {
        ($($type:ty => [$($value:expr),+ $(,)?]),+ $(,)?) => {
            $(
                $(
                    let value: $type = $value;
                    let text = ConvertUtils::integer_to_string(value);
                    let parsed: $type = ConvertUtils::string_to_integer(&text).unwrap();
                    assert_eq!(parsed, value);
                )+
            )+
        };
    }

    assert_round_trip!(
        i8 => [i8::MIN, -1, 0, i8::MAX],
        i16 => [i16::MIN, -1, 0, i16::MAX],
        i32 => [i32::MIN, -1, 0, i32::MAX],
        i64 => [i64::MIN, -1, 0, i64::MAX],
        i128 => [i128::MIN, -1, 0, i128::MAX],
        isize => [isize::MIN, -1, 0, isize::MAX],
        u8 => [0, 1, u8::MAX],
        u16 => [0, 1, u16::MAX],
        u32 => [0, 1, u32::MAX],
        u64 => [0, 1, u64::MAX],
        u128 => [0, 1, u128::MAX],
        usize => [0, 1, usize::MAX],
    );

    let mut borrowed_buffer = IntegerBuffer::new();
    assert_eq!(
        ConvertUtils::integer_to_str(i128::MIN, &mut borrowed_buffer),
        "-170141183460469231731687303715884105728"
    );

    let mut output = String::from("prefix:");
    ConvertUtils::append_integer(&mut output, u64::MAX);
    assert_eq!(output, format!("prefix:{}", u64::MAX));

    let mut root_buffer = axutils::IntegerBuffer::new();
    let mut module_buffer = axutils::convert::IntegerBuffer::new();
    assert_eq!(
        <i32 as axutils::IntegerValue>::format_into(1, &mut root_buffer),
        "1"
    );
    assert_eq!(
        <i32 as axutils::convert::IntegerValue>::format_into(2, &mut module_buffer),
        "2"
    );
    let _ = <u8 as IntegerValue>::format_into;
    assert!(ConvertUtils::string_to_integer::<u8>("").is_err());
    assert!(ConvertUtils::string_to_integer::<u8>("-1").is_err());
    assert!(ConvertUtils::string_to_integer::<i8>("-129").is_err());
    assert!(ConvertUtils::string_to_integer::<u8>("256").is_err());
    assert!(ConvertUtils::string_to_integer::<i32>(" 1").is_err());
}

#[cfg(any(feature = "ryu", feature = "zmij"))]
#[allow(clippy::vec_init_then_push)]
fn float_formats() -> Vec<axutils::FloatFormat> {
    let mut formats = Vec::new();
    #[cfg(feature = "ryu")]
    formats.push(axutils::FloatFormat::Ryu);
    #[cfg(feature = "zmij")]
    formats.push(axutils::FloatFormat::Zmij);
    formats
}

#[cfg(any(feature = "ryu", feature = "zmij"))]
#[test]
fn float_conversion_uses_one_common_api_for_each_enabled_backend() {
    use axutils::{FloatBuffer, FloatValue};

    for format in float_formats() {
        for value in [0.0_f64, -0.0, 1.25, -123.5, 1.0e-30, 1.0e30] {
            let text = ConvertUtils::float_to_string(value, format);
            let parsed: f64 = ConvertUtils::string_to_float(&text).unwrap();
            assert_eq!(parsed.to_bits(), value.to_bits(), "backend text: {text}");

            let mut output = String::with_capacity(32);
            ConvertUtils::append_float(&mut output, value, format);
            assert_eq!(output, text);
        }

        for value in [f32::MIN_POSITIVE, f32::MAX, -f32::MAX] {
            let text = ConvertUtils::float_to_string(value, format);
            let parsed: f32 = ConvertUtils::string_to_float(&text).unwrap();
            assert_eq!(parsed.to_bits(), value.to_bits(), "backend text: {text}");
        }

        for value in [f64::MIN_POSITIVE, f64::MAX, -f64::MAX] {
            let text = ConvertUtils::float_to_string(value, format);
            let parsed: f64 = ConvertUtils::string_to_float(&text).unwrap();
            assert_eq!(parsed.to_bits(), value.to_bits(), "backend text: {text}");
        }

        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let text = ConvertUtils::float_to_string(value, format);
            let parsed: f64 = ConvertUtils::string_to_float(&text).unwrap();
            if value.is_nan() {
                assert!(parsed.is_nan(), "backend text: {text}");
            } else {
                assert_eq!(parsed, value, "backend text: {text}");
            }
        }
        assert_eq!(ConvertUtils::float_to_string(-0.0_f64, format), "-0.0");

        let mut borrowed = FloatBuffer::new(format);
        assert_eq!(ConvertUtils::float_to_str(2.5_f32, &mut borrowed), "2.5");
        assert_eq!(<f64 as FloatValue>::format_into(2.5, &mut borrowed), "2.5");
    }

    let text = ConvertUtils::float_to_string(1.25_f64, {
        #[cfg(feature = "ryu")]
        {
            axutils::FloatFormat::Ryu
        }
        #[cfg(all(not(feature = "ryu"), feature = "zmij"))]
        {
            axutils::FloatFormat::Zmij
        }
    });
    assert_eq!(text, "1.25");
    assert!(ConvertUtils::string_to_float::<f64>("").is_err());
    assert!(ConvertUtils::string_to_float::<f64>(" 1.0").is_err());
    assert!(ConvertUtils::string_to_float::<f64>("1.0 ").is_err());
    assert_eq!(
        ConvertUtils::string_to_float::<f64>("1e9999").unwrap(),
        f64::INFINITY
    );
    assert_eq!(
        ConvertUtils::string_to_float::<f64>("1e-9999").unwrap(),
        0.0
    );
}

#[cfg(feature = "uuid")]
#[test]
fn uuid_conversion_uses_root_and_convert_paths_and_documented_input_forms() {
    use axutils::UuidBuffer;

    const CANONICAL: &str = "550e8400-e29b-41d4-a716-446655440000";
    let uuid = ConvertUtils::string_to_uuid(CANONICAL).unwrap();

    let mut buffer = UuidBuffer::default();
    assert_eq!(ConvertUtils::uuid_to_str(&uuid, &mut buffer), CANONICAL);

    let mut output = String::from("id=");
    ConvertUtils::append_uuid(&mut output, &uuid);
    assert_eq!(output, format!("id={CANONICAL}"));
    assert_eq!(ConvertUtils::uuid_to_string(&uuid), CANONICAL);

    for input in [
        CANONICAL,
        "550e8400e29b41d4a716446655440000",
        "urn:uuid:550e8400-e29b-41d4-a716-446655440000",
        "{550e8400-e29b-41d4-a716-446655440000}",
        "550E8400-E29B-41D4-A716-446655440000",
    ] {
        assert_eq!(ConvertUtils::string_to_uuid(input).unwrap(), uuid);
    }
    for input in [
        "",
        "550e8400-e29b-41d4-a716-44665544000",
        "550e8400-e29b-41d4-a716-44665544000g",
        "550e8400_e29b-41d4-a716-446655440000",
        " 550e8400-e29b-41d4-a716-446655440000",
    ] {
        assert!(ConvertUtils::string_to_uuid(input).is_err());
    }

    let _: axutils::UuidBuffer = axutils::UuidBuffer::new();
    let _: axutils::convert::UuidBuffer = axutils::convert::UuidBuffer::new();
}
