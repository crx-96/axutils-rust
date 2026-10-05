//! 进程内并发准入；只管理调用方共享实例中的键和名额，不提供分布式互斥。

mod keyed_admission;

pub use keyed_admission::{AdmissionError, KeyedAdmission, KeyedPermit};
