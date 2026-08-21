#[cfg(feature = "both")]
fn main() {
    use axutils::RegUtils;

    let _: fn(&str) -> bool = RegUtils::is_phone;
    let _: fn(&str) -> bool = axutils::reg_utils::RegUtils::is_phone;
    let _: fn(&str) -> bool = axutils::utils::RegUtils::is_phone;
    let _: fn(&str) -> bool = axutils::utils::reg_utils::RegUtils::is_phone;
    assert!(RegUtils::is_phone("+8613812345678"));
    assert!(!RegUtils::is_phone("8613812345678"));
    assert!(!RegUtils::is_phone("+86 13812345678"));
    assert!(!RegUtils::is_phone("+1234567890123456"));
}

#[cfg(feature = "negative-regex-only-phone")]
fn main() {
    let _ = axutils::RegUtils::is_phone;
}

#[cfg(feature = "negative-libphonenumber-only-reg")]
fn main() {
    let _ = axutils::RegUtils::is_email;
}

#[cfg(feature = "negative-none-reg")]
fn main() {
    let _ = axutils::RegUtils::is_email;
}

#[cfg(not(any(
    feature = "both",
    feature = "negative-regex-only-phone",
    feature = "negative-libphonenumber-only-reg",
    feature = "negative-none-reg",
)))]
fn main() {}
