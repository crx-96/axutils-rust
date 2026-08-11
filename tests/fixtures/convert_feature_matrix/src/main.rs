#[cfg(any(
    feature = "itoa-only",
    feature = "itoa-ryu",
    feature = "itoa-zmij",
    feature = "itoa-uuid",
    feature = "itoa-ryu-zmij",
    feature = "itoa-ryu-uuid",
    feature = "itoa-zmij-uuid",
    feature = "all",
))]
fn check_integer_api() {
    use axutils::{IntegerBuffer, IntegerValue};

    let mut root_buffer = axutils::IntegerBuffer::new();
    let mut module_buffer = axutils::convert::IntegerBuffer::default();
    assert_eq!(
        axutils::ConvertUtils::integer_to_str(42_i32, &mut root_buffer),
        "42"
    );
    assert_eq!(
        axutils::convert::ConvertUtils::integer_to_str(43_i32, &mut module_buffer),
        "43"
    );
    let mut output = String::new();
    axutils::ConvertUtils::append_integer(&mut output, i128::MIN);
    assert_eq!(output, i128::MIN.to_string());
    assert_eq!(
        axutils::ConvertUtils::integer_to_string(u128::MAX),
        u128::MAX.to_string()
    );
    let parsed: i64 = axutils::ConvertUtils::string_to_integer("-64").unwrap();
    assert_eq!(parsed, -64);
    assert_eq!(
        <i32 as axutils::IntegerValue>::format_into(46, &mut root_buffer),
        "46"
    );
    assert_eq!(
        <i32 as axutils::convert::IntegerValue>::format_into(47, &mut module_buffer),
        "47"
    );
    let _ = <u8 as IntegerValue>::format_into;
    let _: axutils::IntegerBuffer = IntegerBuffer::new();
}

#[cfg(any(
    feature = "ryu-only",
    feature = "zmij-only",
    feature = "itoa-ryu",
    feature = "itoa-zmij",
    feature = "ryu-zmij",
    feature = "ryu-uuid",
    feature = "zmij-uuid",
    feature = "itoa-ryu-zmij",
    feature = "itoa-ryu-uuid",
    feature = "itoa-zmij-uuid",
    feature = "ryu-zmij-uuid",
    feature = "all",
))]
fn check_float_api() {
    use axutils::{FloatBuffer, FloatFormat, FloatValue};

    #[cfg(any(
        feature = "ryu-only",
        feature = "itoa-ryu",
        feature = "ryu-zmij",
        feature = "ryu-uuid",
        feature = "itoa-ryu-zmij",
        feature = "itoa-ryu-uuid",
        feature = "ryu-zmij-uuid",
        feature = "all",
    ))]
    {
        let mut root_buffer = axutils::FloatBuffer::new(axutils::FloatFormat::Ryu);
        let mut module_buffer = axutils::convert::FloatBuffer::new(axutils::FloatFormat::Ryu);
        assert_eq!(
            axutils::ConvertUtils::float_to_str(1.25_f64, &mut root_buffer),
            "1.25"
        );
        assert_eq!(
            axutils::convert::ConvertUtils::float_to_str(1.5_f64, &mut module_buffer),
            "1.5"
        );
        assert_eq!(
            <f32 as axutils::FloatValue>::format_into(2.5, &mut root_buffer),
            "2.5"
        );
        let _ = axutils::FloatFormat::Ryu;
    }

    #[cfg(any(
        feature = "zmij-only",
        feature = "itoa-zmij",
        feature = "ryu-zmij",
        feature = "zmij-uuid",
        feature = "itoa-ryu-zmij",
        feature = "itoa-zmij-uuid",
        feature = "ryu-zmij-uuid",
        feature = "all",
    ))]
    {
        let mut root_buffer = axutils::FloatBuffer::new(axutils::FloatFormat::Zmij);
        let mut module_buffer = axutils::convert::FloatBuffer::new(axutils::FloatFormat::Zmij);
        assert_eq!(
            axutils::ConvertUtils::float_to_str(1.25_f64, &mut root_buffer),
            "1.25"
        );
        assert_eq!(
            axutils::convert::ConvertUtils::float_to_str(1.5_f64, &mut module_buffer),
            "1.5"
        );
        assert_eq!(
            <f64 as axutils::convert::FloatValue>::format_into(2.5, &mut root_buffer),
            "2.5"
        );
        let _ = axutils::FloatFormat::Zmij;
    }

    let format = {
        #[cfg(any(
            feature = "ryu-only",
            feature = "itoa-ryu",
            feature = "ryu-zmij",
            feature = "ryu-uuid",
            feature = "itoa-ryu-zmij",
            feature = "itoa-ryu-uuid",
            feature = "ryu-zmij-uuid",
            feature = "all",
        ))]
        {
            FloatFormat::Ryu
        }
        #[cfg(all(
            not(any(
                feature = "ryu-only",
                feature = "itoa-ryu",
                feature = "ryu-zmij",
                feature = "ryu-uuid",
                feature = "itoa-ryu-zmij",
                feature = "itoa-ryu-uuid",
                feature = "ryu-zmij-uuid",
                feature = "all",
            )),
            any(
                feature = "zmij-only",
                feature = "itoa-zmij",
                feature = "zmij-uuid",
                feature = "itoa-zmij-uuid",
            ),
        ))]
        {
            FloatFormat::Zmij
        }
    };
    let mut output = String::new();
    axutils::ConvertUtils::append_float(&mut output, -0.5_f64, format);
    assert_eq!(output, "-0.5");
    assert_eq!(
        axutils::ConvertUtils::float_to_string(1.25_f64, format),
        "1.25"
    );
    let parsed: f64 = axutils::ConvertUtils::string_to_float("1.25").unwrap();
    assert_eq!(parsed, 1.25);
    let _: axutils::FloatBuffer = FloatBuffer::new(format);
    let _ = <f64 as FloatValue>::format_into;
}

#[cfg(any(
    feature = "uuid-only",
    feature = "itoa-uuid",
    feature = "ryu-uuid",
    feature = "zmij-uuid",
    feature = "itoa-ryu-uuid",
    feature = "itoa-zmij-uuid",
    feature = "ryu-zmij-uuid",
    feature = "all",
))]
fn check_uuid_api() {
    use axutils::UuidBuffer;

    let uuid = axutils::ConvertUtils::string_to_uuid(
        "550e8400-e29b-41d4-a716-446655440000",
    )
    .unwrap();
    let mut root_buffer = axutils::UuidBuffer::new();
    let mut module_buffer = axutils::convert::UuidBuffer::new();
    assert_eq!(
        axutils::ConvertUtils::uuid_to_str(&uuid, &mut root_buffer),
        "550e8400-e29b-41d4-a716-446655440000"
    );
    assert_eq!(
        axutils::convert::ConvertUtils::uuid_to_str(&uuid, &mut module_buffer),
        "550e8400-e29b-41d4-a716-446655440000"
    );
    let mut output = String::new();
    axutils::ConvertUtils::append_uuid(&mut output, &uuid);
    assert_eq!(output, axutils::ConvertUtils::uuid_to_string(&uuid));
    let _: axutils::UuidBuffer = UuidBuffer::default();
    let _ = uuid::Uuid::nil();
}

#[cfg(not(any(
    feature = "negative-no-itoa-integer",
    feature = "negative-no-float",
    feature = "negative-no-uuid",
    feature = "negative-no-ryu-variant",
    feature = "negative-no-zmij-variant",
    feature = "negative-float-default",
    feature = "negative-float-suffix",
    feature = "negative-integer-custom",
    feature = "negative-float-custom",
    feature = "negative-integer-sealed",
    feature = "negative-float-sealed",
    feature = "negative-utils-domain-types",
)))]
fn main() {
    #[cfg(any(
        feature = "itoa-only",
        feature = "itoa-ryu",
        feature = "itoa-zmij",
        feature = "itoa-uuid",
        feature = "itoa-ryu-zmij",
        feature = "itoa-ryu-uuid",
        feature = "itoa-zmij-uuid",
        feature = "all",
    ))]
    check_integer_api();

    #[cfg(any(
        feature = "ryu-only",
        feature = "zmij-only",
        feature = "itoa-ryu",
        feature = "itoa-zmij",
        feature = "ryu-zmij",
        feature = "ryu-uuid",
        feature = "zmij-uuid",
        feature = "itoa-ryu-zmij",
        feature = "itoa-ryu-uuid",
        feature = "itoa-zmij-uuid",
        feature = "ryu-zmij-uuid",
        feature = "all",
    ))]
    check_float_api();

    #[cfg(any(
        feature = "uuid-only",
        feature = "itoa-uuid",
        feature = "ryu-uuid",
        feature = "zmij-uuid",
        feature = "itoa-ryu-uuid",
        feature = "itoa-zmij-uuid",
        feature = "ryu-zmij-uuid",
        feature = "all",
    ))]
    check_uuid_api();
}

#[cfg(feature = "negative-no-itoa-integer")]
fn main() {
    let _ = axutils::ConvertUtils::integer_to_string(1_i32);
}

#[cfg(feature = "negative-no-float")]
fn main() {
    let _ = axutils::ConvertUtils::float_to_string::<f64>(1.0, ());
}

#[cfg(feature = "negative-no-uuid")]
fn main() {
    let _ = axutils::ConvertUtils::string_to_uuid("00000000-0000-0000-0000-000000000000");
}

#[cfg(feature = "negative-no-ryu-variant")]
fn main() {
    let _ = axutils::FloatFormat::Ryu;
}

#[cfg(feature = "negative-no-zmij-variant")]
fn main() {
    let _ = axutils::FloatFormat::Zmij;
}

#[cfg(feature = "negative-float-default")]
fn main() {
    let _ = axutils::FloatBuffer::default();
}

#[cfg(feature = "negative-float-suffix")]
fn main() {
    let _ = axutils::ConvertUtils::float_to_string_ryu;
}

#[cfg(feature = "negative-integer-custom")]
fn main() {
    struct Custom;
    let _ = axutils::ConvertUtils::integer_to_string(Custom);
}

#[cfg(feature = "negative-float-custom")]
fn main() {
    struct Custom;
    let _ = axutils::ConvertUtils::float_to_string(Custom, axutils::FloatFormat::Ryu);
}

#[cfg(feature = "negative-integer-sealed")]
fn main() {
    struct Custom;
    impl axutils::IntegerValue for Custom {
        fn format_into<'a>(_: Self, _: &'a mut axutils::IntegerBuffer) -> &'a str {
            "custom"
        }
    }
    let _ = Custom;
}

#[cfg(feature = "negative-float-sealed")]
fn main() {
    struct Custom;
    impl axutils::FloatValue for Custom {
        fn format_into<'a>(_: Self, _: &'a mut axutils::FloatBuffer) -> &'a str {
            "custom"
        }
    }
    let _ = Custom;
}

#[cfg(feature = "negative-utils-domain-types")]
fn main() {
    let _ = axutils::utils::ConvertUtils;
    let _ = axutils::utils::convert_utils::ConvertUtils;
    let _ = axutils::utils::IntegerBuffer::new();
    let _ = axutils::utils::convert_utils::FloatBuffer::new(axutils::FloatFormat::Ryu);
    let _ = axutils::utils::UuidBuffer::new();
}
