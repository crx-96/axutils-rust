//! AES 模式元数据与共享输入下限。

/// AES 加解密模式。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AesMode {
    /// AES-GCM：带认证的加密，12 字节 nonce、16 字节标签。推荐默认模式。
    Gcm,
    /// AES-CBC + PKCS#7：**无完整性认证**，16 字节 IV。仅用于与旧系统互操作；新系统应使用
    /// [`Gcm`](AesMode::Gcm)，且必须由上层协议自行提供认证。
    CbcPkcs7,
}

impl AesMode {
    /// 返回本模式要求的 IV/nonce 长度（字节）。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::crypto::AesMode;
    ///
    /// assert_eq!(AesMode::Gcm.iv_length(), 12);
    /// assert_eq!(AesMode::CbcPkcs7.iv_length(), 16);
    /// ```
    #[must_use]
    pub fn iv_length(&self) -> usize {
        match self {
            Self::Gcm => 12,
            Self::CbcPkcs7 => 16,
        }
    }

    /// 返回本模式是否提供认证（完整性保护）。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::crypto::AesMode;
    ///
    /// assert!(AesMode::Gcm.is_authenticated());
    /// assert!(!AesMode::CbcPkcs7.is_authenticated());
    /// ```
    #[must_use]
    pub fn is_authenticated(&self) -> bool {
        matches!(self, Self::Gcm)
    }

    /// 返回模式名称。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::crypto::AesMode;
    ///
    /// assert_eq!(AesMode::Gcm.as_str(), "AES-GCM");
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Gcm => "AES-GCM",
            Self::CbcPkcs7 => "AES-CBC-PKCS7",
        }
    }

    pub(super) fn min_body_length(&self) -> usize {
        16
    }
}
