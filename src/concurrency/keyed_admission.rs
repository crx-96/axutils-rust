use std::{
    collections::HashSet,
    error::Error,
    fmt,
    sync::{Arc, Mutex, PoisonError},
};

/// 按字符串键去重并限制总占用量的进程内准入集合。
///
/// 克隆实例共享键集合和容量；分别调用 [`Self::new`] 创建的实例相互独立。
/// 准入不等待名额可用，但取得内部同步互斥锁时可能短暂阻塞。凭证不持有该锁，
/// 可以跨业务操作和 `.await` 持有；键规范化、业务响应和分布式互斥由调用方负责。
#[derive(Clone)]
pub struct KeyedAdmission {
    /// 当前仍被凭证占用的字符串键；所有克隆及凭证共同持有该状态。
    active: Arc<Mutex<HashSet<String>>>,
    /// 最大同时占用的不同键数量；零表示拒绝全部新准入。
    capacity: usize,
}

/// 准入失败的稳定分类，不包含调用方的键或业务数据。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionError {
    /// 同一键已有凭证；在集合未中毒时，此判断优先于总容量检查。
    Busy,
    /// 总占用量已达到容量上限；容量为零时任何新键均返回此项。
    Full,
    /// 内部互斥锁已中毒，后续新准入持续拒绝，避免继续使用不可信状态。
    Unavailable,
}

impl fmt::Display for AdmissionError {
    /// 只输出错误分类，不泄露准入键。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Busy => "the key is already admitted",
            Self::Full => "admission capacity is full",
            Self::Unavailable => "admission state is unavailable",
        };
        formatter.write_str(message)
    }
}

impl Error for AdmissionError {}

impl fmt::Debug for KeyedAdmission {
    /// 调试输出只展示容量，不锁定状态或输出可能包含业务数据的键。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KeyedAdmission")
            .field("capacity", &self.capacity)
            .finish_non_exhaustive()
    }
}

impl KeyedAdmission {
    /// 创建容量为 `capacity` 的空准入集合，不启动任务或访问外部服务。
    ///
    /// 容量为零是有效配置：所有新准入立即返回 [`AdmissionError::Full`]。
    ///
    /// # Examples
    ///
    /// ```rust
    /// use axutils::concurrency::{AdmissionError, KeyedAdmission};
    ///
    /// let admission = KeyedAdmission::new(1);
    /// let permit = admission.try_enter("record").unwrap();
    /// assert!(matches!(admission.try_enter("record"), Err(AdmissionError::Busy)));
    /// drop(permit);
    /// assert!(admission.try_enter("record").is_ok());
    /// ```
    pub fn new(capacity: usize) -> Self {
        Self {
            active: Arc::new(Mutex::new(HashSet::new())),
            capacity,
        }
    }

    /// 尝试独占 `key` 并取得一个名额，接受拥有型或借用的字符串，不规范化键。
    ///
    /// 锁中毒时返回 [`AdmissionError::Unavailable`]；否则同键重复优先返回
    /// [`AdmissionError::Busy`]，容量不足返回 [`AdmissionError::Full`]。不等待名额释放，
    /// 但内部 [`Mutex::lock`] 可能等待其他短暂的准入或释放操作。
    ///
    /// 成功后必须在整个操作期间持有凭证；正常返回、错误、panic unwind 或 future
    /// 被取消并丢弃时，凭证的 `Drop` 归还名额。`mem::forget`、进程终止和 panic abort
    /// 不执行该清理，不能据此保证释放。
    pub fn try_enter(&self, key: impl Into<String>) -> Result<KeyedPermit, AdmissionError> {
        // 先完成调用方字符串转换，再锁定集合，避免在锁内执行用户提供的转换逻辑。
        let key = key.into();
        let mut active = self
            .active
            .lock()
            .map_err(|_| AdmissionError::Unavailable)?;

        // 重复键优先于容量判断，保证集合恰好满额时仍报告同键冲突。
        if active.contains(&key) {
            return Err(AdmissionError::Busy);
        }
        if active.len() >= self.capacity {
            return Err(AdmissionError::Full);
        }

        // 在同一临界区登记占用，返回前释放锁；凭证只保留释放所需的共享状态与键。
        active.insert(key.clone());
        drop(active);
        Ok(KeyedPermit {
            active: Arc::clone(&self.active),
            key,
        })
    }
}

/// 持有一个键和一个名额的唯一凭证，丢弃时自动归还占用。
///
/// 凭证不能克隆，不持有互斥锁；可以移动到其他线程或异步任务。集合锁中毒后，
/// `Drop` 仍恢复锁内状态并移除自己的键，但不会清除中毒标记或重新开放准入。
#[must_use = "操作结束前必须持有准入凭证"]
pub struct KeyedPermit {
    /// 凭证归还键时使用的共享集合，使原准入对象先释放后仍可完成清理。
    active: Arc<Mutex<HashSet<String>>>,
    /// 本凭证独占的键；只在释放占用时访问，不通过调试输出暴露。
    key: String,
}

impl fmt::Debug for KeyedPermit {
    /// 调试输出隐藏键及共享集合内容。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KeyedPermit")
            .finish_non_exhaustive()
    }
}

impl Drop for KeyedPermit {
    /// 回收自己的占用；即使锁中毒也尽力清理，但保留拒绝后续准入的中毒状态。
    fn drop(&mut self) {
        // 清理不因其他持锁操作曾 panic 而再次 panic；不清毒，避免隐式恢复不可信集合。
        self.active
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.key);
    }
}

#[cfg(test)]
#[path = "keyed_admission/tests.rs"]
mod tests;
