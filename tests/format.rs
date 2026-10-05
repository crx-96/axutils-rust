use axutils::utils::FormatUtils;

#[test]
fn html_escaping_preserves_unicode_and_escapes_all_special_characters() {
    assert_eq!(
        FormatUtils::escape_html("中文🙂<&>\"' e\u{301}"),
        "中文🙂&lt;&amp;&gt;&quot;&#39; e\u{301}",
    );
    assert_eq!(FormatUtils::escape_html(""), "");
    assert_eq!(FormatUtils::escape_html("普通文字🙂"), "普通文字🙂");
}

#[test]
fn html_escaping_treats_existing_entities_as_input_text() {
    assert_eq!(FormatUtils::escape_html("&lt;&"), "&amp;lt;&amp;");
}

#[test]
fn replacements_preserve_unknown_markers_and_unregistered_braces() {
    assert_eq!(
        FormatUtils::replace_placeholders(
            "样式 { color: red } {a}/{b}/{a}/{unknown} 🙂",
            &[("{a}", "{b}"), ("{b}", "完成"), ("", "忽略")],
        ),
        "样式 { color: red } {b}/完成/{b}/{unknown} 🙂",
    );
}

#[test]
fn replacements_ignore_empty_markers_and_handle_empty_inputs() {
    assert_eq!(
        FormatUtils::replace_placeholders("原文", &[("", "忽略")]),
        "原文"
    );
    assert_eq!(FormatUtils::replace_placeholders("", &[("x", "y")]), "");
    assert_eq!(FormatUtils::replace_placeholders("{a}", &[]), "{a}");
    assert_eq!(
        FormatUtils::replace_placeholders("删除{a}保留", &[("{a}", "")]),
        "删除保留",
    );
}

#[test]
fn replacements_prioritize_earliest_position_before_table_order() {
    assert_eq!(
        FormatUtils::replace_placeholders("甲乙丙丁", &[("丙丁", "后"), ("甲乙", "先")]),
        "先后",
    );
}

#[test]
fn duplicate_and_overlapping_markers_follow_table_order_at_same_position() {
    assert_eq!(
        FormatUtils::replace_placeholders("abcabc", &[("ab", "短"), ("abc", "长"), ("ab", "重复")]),
        "短c短c",
    );
    assert_eq!(
        FormatUtils::replace_placeholders("abcabc", &[("abc", "长"), ("ab", "短")]),
        "长长",
    );
    assert_eq!(
        FormatUtils::replace_placeholders("ababa", &[("aba", "X"), ("bab", "Y")]),
        "Xba",
    );
}

#[test]
fn replacements_support_borrowed_unicode_markers_and_values() {
    let marker = String::from("名字🙂");
    let value = String::from("李雷");
    assert_eq!(
        FormatUtils::replace_placeholders("你好，名字🙂！", &[(marker.as_str(), value.as_str())]),
        "你好，李雷！",
    );
}

#[test]
fn replacement_values_are_neither_reexpanded_nor_escaped() {
    assert_eq!(
        FormatUtils::replace_placeholders("{a}{b}", &[("{a}", "{b}<>&\"'"), ("{b}", "完成")]),
        "{b}<>&\"'完成",
    );
}
