use super::FormatUtils;

impl FormatUtils {
    /// 按字符位置对字符串执行一段或多段脱敏。
    ///
    /// `ranges` 中的每一项都是零基、左闭右开的 `(start, end)` 字符范围，按 Unicode 标量值
    /// 而不是 UTF-8 字节计数。范围必须按升序排列、互不重叠、至少包含一个字符且不能越界；
    /// 任一范围无效或结果内存预留失败时返回 `None`。空范围列表会返回输入的拥有副本。
    /// `replacement` 为 `None` 时，每一段统一替换为 `"****"`；传入空字符串可删除指定段。
    ///
    /// 时间复杂度为 `O(n + r)`，其中 `n` 是输入字节数，`r` 是范围数量；实现会为字符边界和
    /// 返回字符串分配与输入及输出长度相称的内存。调用方应限制不可信输入、范围数量和替换串
    /// 长度。该方法只按位置替换内容，不识别字段语义，也不保证输入的其他副本已被清除。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::FormatUtils;
    ///
    /// assert_eq!(
    ///     FormatUtils::mask("13812345678", &[(3, 7)], None),
    ///     Some("138****5678".to_owned()),
    /// );
    /// assert_eq!(
    ///     FormatUtils::mask("甲乙丙丁戊己", &[(1, 3), (4, 6)], Some("#")),
    ///     Some("甲#丁#".to_owned()),
    /// );
    /// assert_eq!(FormatUtils::mask("abc", &[(2, 1)], None), None);
    /// ```
    pub fn mask(
        value: &str,
        ranges: &[(usize, usize)],
        replacement: Option<&str>,
    ) -> Option<String> {
        let replacement = replacement.unwrap_or("****");
        let character_count = value.chars().count();
        let boundary_capacity = character_count.checked_add(1)?;
        let mut byte_offsets = Vec::new();
        byte_offsets.try_reserve_exact(boundary_capacity).ok()?;
        byte_offsets.push(0);
        byte_offsets.extend(value.char_indices().skip(1).map(|(offset, _)| offset));
        if !value.is_empty() {
            byte_offsets.push(value.len());
        }

        let mut previous_end = 0;
        let mut masked_bytes = 0usize;
        for &(start, end) in ranges {
            if start >= end || start < previous_end || end > character_count {
                return None;
            }
            masked_bytes = masked_bytes.checked_add(byte_offsets[end] - byte_offsets[start])?;
            previous_end = end;
        }

        let replacements_bytes = replacement.len().checked_mul(ranges.len())?;
        let output_capacity = value
            .len()
            .checked_sub(masked_bytes)?
            .checked_add(replacements_bytes)?;
        let mut output = String::new();
        output.try_reserve_exact(output_capacity).ok()?;

        let mut copied_until = 0;
        for &(start, end) in ranges {
            output.push_str(&value[copied_until..byte_offsets[start]]);
            output.push_str(replacement);
            copied_until = byte_offsets[end];
        }
        output.push_str(&value[copied_until..]);
        Some(output)
    }

    /// 从指定字符位置开始对邮箱地址的本地部分执行脱敏。
    ///
    /// 输入必须恰好包含一个 `@`，且本地部分和域名都非空，否则返回 `None`。本地部分包含多个
    /// Unicode 字符时，`start` 使用一基位置；`None` 默认从第 4 个字符开始脱敏。如果本地部分
    /// 的字符数小于指定位置，则改为从第 1 个字符开始全部脱敏。位置 `0` 无效并返回 `None`。
    /// 域名保持不变，成功时返回拥有的脱敏字符串。
    ///
    /// 本方法只检查上述可安全拆分的结构，不负责 RFC 邮箱语法、域名或邮箱真实性校验；如需
    /// 格式校验，应在启用 `regex` feature 后另行使用 `RegUtils`。实现会分配与输入和输出长度
    /// 相称的内存，内存预留失败时返回 `None`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::FormatUtils;
    ///
    /// assert_eq!(
    ///     FormatUtils::mask_email("alice@example.com", None),
    ///     Some("ali****@example.com".to_owned()),
    /// );
    /// assert_eq!(
    ///     FormatUtils::mask_email("alice@example.com", Some(2)),
    ///     Some("a****@example.com".to_owned()),
    /// );
    /// assert_eq!(
    ///     FormatUtils::mask_email("李雷@example.com", None),
    ///     Some("****@example.com".to_owned()),
    /// );
    /// assert_eq!(FormatUtils::mask_email("invalid", None), None);
    /// ```
    pub fn mask_email(email: &str, start: Option<usize>) -> Option<String> {
        let (local, domain) = email.split_once('@')?;
        if local.is_empty() || domain.is_empty() || domain.contains('@') {
            return None;
        }

        let start_position = start.unwrap_or(4);
        if start_position == 0 {
            return None;
        }
        let local_character_count = local.chars().count();
        let start_index = if local_character_count < start_position {
            0
        } else {
            start_position - 1
        };
        Self::mask(email, &[(start_index, local_character_count)], None)
    }
}
#[cfg(test)]
mod tests {
    use super::FormatUtils;

    #[test]
    fn mask_uses_default_replacement_for_one_range() {
        assert_eq!(
            FormatUtils::mask("13812345678", &[(3, 7)], None),
            Some("138****5678".to_owned())
        );
    }

    #[test]
    fn mask_supports_unicode_multiple_ranges_and_custom_replacement() {
        assert_eq!(
            FormatUtils::mask("甲乙丙丁戊己", &[(1, 3), (4, 6)], Some("[隐藏]")),
            Some("甲[隐藏]丁[隐藏]".to_owned())
        );
        assert_eq!(
            FormatUtils::mask("abcde", &[(0, 2), (2, 4)], Some("*")),
            Some("**e".to_owned())
        );
    }

    #[test]
    fn mask_accepts_empty_ranges_and_empty_replacement() {
        assert_eq!(
            FormatUtils::mask("原文", &[], None),
            Some("原文".to_owned())
        );
        assert_eq!(
            FormatUtils::mask("abcdef", &[(1, 3), (4, 6)], Some("")),
            Some("ad".to_owned())
        );
    }

    #[test]
    fn mask_rejects_invalid_ranges() {
        for ranges in [
            &[(1, 1)][..],
            &[(2, 1)][..],
            &[(0, 4)][..],
            &[(2, 3), (1, 2)][..],
            &[(0, 2), (1, 3)][..],
        ] {
            assert_eq!(FormatUtils::mask("abc", ranges, None), None);
        }
    }

    #[test]
    fn mask_email_preserves_domain_and_masks_local_part() {
        assert_eq!(
            FormatUtils::mask_email("alice+tag@example.com", None),
            Some("ali****@example.com".to_owned())
        );
        assert_eq!(
            FormatUtils::mask_email("李雷@example.com", None),
            Some("****@example.com".to_owned())
        );
        assert_eq!(
            FormatUtils::mask_email("a@example.com", None),
            Some("****@example.com".to_owned())
        );
        assert_eq!(
            FormatUtils::mask_email("abcd@example.com", None),
            Some("abc****@example.com".to_owned())
        );
        assert_eq!(
            FormatUtils::mask_email("alice@example.com", Some(2)),
            Some("a****@example.com".to_owned())
        );
        assert_eq!(
            FormatUtils::mask_email("alice@example.com", Some(5)),
            Some("alic****@example.com".to_owned())
        );
        assert_eq!(
            FormatUtils::mask_email("alice@example.com", Some(6)),
            Some("****@example.com".to_owned())
        );
    }

    #[test]
    fn mask_email_rejects_ambiguous_or_incomplete_addresses() {
        for email in ["", "invalid", "@example.com", "alice@", "a@b@example.com"] {
            assert_eq!(FormatUtils::mask_email(email, None), None);
        }
        assert_eq!(FormatUtils::mask_email("alice@example.com", Some(0)), None);
    }
}
