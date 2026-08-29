#![allow(clippy::too_many_lines)]

//! Markdown 示例的离线编译闭环。
//!
//! 该测试默认只做枚举和 metadata 双向校验；编译是 ignored 测试。局部验证通过
//! `AXUTILS_DOCS_EXAMPLE_FILTER` 选择受影响文档，完整验证才不设置过滤器。
//! 每个 Rust fence 都在独立的临时 crate 中检查，所有临时 crate 共用本测试专属的
//! `CARGO_TARGET_DIR`，并且由本测试串行执行，避免并发 rustdoc/cargo 产物相互污染。

use std::collections::BTreeSet;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const MODULE_MAP: &str = include_str!("../docs/module-map.md");
const ROOT_MANIFEST: &str = env!("CARGO_MANIFEST_DIR");
const VALID_AXUTILS_FEATURES: &[&str] = &[
    "itoa",
    "ryu",
    "zmij",
    "uuid",
    "jwt",
    "tracing",
    "logging",
    "rand",
    "regex",
    "libphonenumber",
    "serde",
    "strfmt",
    "minijinja",
    "chrono",
    "chrono_tz",
    "croner",
    "time",
    "jiff",
    "lettre",
    "http",
    "redis",
    "sqlx",
    "tokio",
    "axum",
    "tokio-util",
    "tower",
    "tower-http",
    "tower_governor",
    "tempfile",
    "tempfile-async",
    "toml",
    "serde-saphyr",
    "rust-ini",
    "base64",
    "md5",
    "aes",
    "encoding_rs",
    "mimalloc",
    "rpmalloc",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompileMode {
    Compiled,
    NoRun,
    ExplicitlyExcluded,
}

impl CompileMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Compiled => "compiled",
            Self::NoRun => "no_run",
            Self::ExplicitlyExcluded => "explicitly_excluded",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct DirectDependency {
    name: &'static str,
    package: Option<&'static str>,
    version: &'static str,
    default_features: bool,
    features: &'static [&'static str],
}

#[derive(Clone, Copy, Debug)]
struct BlockMetadata {
    key: &'static str,
    axutils_features: &'static [&'static str],
    direct_dependencies: &'static [DirectDependency],
    mode: CompileMode,
    exclusion_reason: Option<&'static str>,
}

#[derive(Debug)]
struct Fence {
    info: String,
    body: String,
}

#[derive(Debug)]
struct Document {
    path: String,
    fences: Vec<Fence>,
}

// This table is intentionally checked in: it is the per-block contract, not a document-level
// feature guess. The generated-looking repetition makes reviewable keys stable and prevents a
// newly added fence from silently disappearing from the compile loop.
const F_NONE: &[&str] = &[];
const F_RANDOM: &[&str] = &["rand"];
const F_SQLX: &[&str] = &["sqlx", "tokio"];
const F_LOG: &[&str] = &["logging"];
const F_SCHEDULER: &[&str] = &["chrono", "chrono_tz", "croner", "tokio"];
const F_AXUM_BASE: &[&str] = &["axum", "tokio"];
const F_AXUM_TOWER: &[&str] = &["axum", "tokio", "tower"];
const F_AXUM_HTTP: &[&str] = &["axum", "tokio", "tower-http"];
const F_AXUM_GOVERNOR: &[&str] = &["axum", "tokio", "tower_governor"];
const F_AXUM_TRACE: &[&str] = &["axum", "tokio", "tower-http", "tracing"];

const DEP_TOKIO: DirectDependency = DirectDependency {
    name: "tokio",
    package: None,
    version: "1",
    default_features: false,
    features: &["macros", "rt-multi-thread", "net", "time", "sync", "signal"],
};
const DEP_CHRONO: DirectDependency = DirectDependency {
    name: "chrono",
    package: None,
    version: "0.4",
    default_features: false,
    features: &["clock"],
};
const DEP_TIME: DirectDependency = DirectDependency {
    name: "time",
    package: None,
    version: "0.3",
    default_features: false,
    features: &["formatting", "macros", "parsing"],
};
const DEP_JIFF: DirectDependency = DirectDependency {
    name: "jiff",
    package: None,
    version: "0.2",
    default_features: true,
    features: &[],
};
const DEP_SERDE: DirectDependency = DirectDependency {
    name: "serde",
    package: None,
    version: "1",
    default_features: false,
    features: &["derive"],
};
const DEP_UUID: DirectDependency = DirectDependency {
    name: "uuid",
    package: None,
    version: "1",
    default_features: false,
    features: &["std"],
};
const DEP_SQLX: DirectDependency = DirectDependency {
    name: "sqlx",
    package: None,
    version: "0.9",
    default_features: false,
    features: &["any", "postgres", "mysql", "sqlite", "runtime-tokio"],
};
const DEP_AXUM: DirectDependency = DirectDependency {
    name: "axum",
    package: None,
    version: "0.8",
    default_features: false,
    features: &["tokio", "http1"],
};
const DEP_TOWER: DirectDependency = DirectDependency {
    name: "tower",
    package: None,
    version: "0.5",
    default_features: false,
    features: &["util", "limit", "load-shed"],
};

// Backend-neutral TimeUtils fences contain a cfg branch for each supported backend. The primary
// table checks chrono; these checked-in combinations also compile the time and jiff branches.
const ADDITIONAL_COMPILE_COMBINATIONS: &[BlockMetadata] = &[
    BlockMetadata {
        key: "docs/examples/time.md#36",
        axutils_features: &["time"],
        direct_dependencies: &[DEP_TIME],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#36",
        axutils_features: &["jiff"],
        direct_dependencies: &[DEP_JIFF],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#37",
        axutils_features: &["time"],
        direct_dependencies: &[DEP_TIME],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#37",
        axutils_features: &["jiff"],
        direct_dependencies: &[DEP_JIFF],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#38",
        axutils_features: &["time"],
        direct_dependencies: &[DEP_TIME],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#38",
        axutils_features: &["jiff"],
        direct_dependencies: &[DEP_JIFF],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#39",
        axutils_features: &["time"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#39",
        axutils_features: &["jiff"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#40",
        axutils_features: &["time"],
        direct_dependencies: &[DEP_TIME],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#40",
        axutils_features: &["jiff"],
        direct_dependencies: &[DEP_JIFF],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#41",
        axutils_features: &["time"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#41",
        axutils_features: &["jiff"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
];

const BLOCK_METADATA: &[BlockMetadata] = &[
    BlockMetadata {
        key: "docs/examples/fs.md#1",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/fs.md#2",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/fs.md#3",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/fs.md#4",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#5",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#6",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#7",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#8",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#9",
        axutils_features: &["tempfile", "tempfile-async"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#10",
        axutils_features: &["tempfile", "tempfile-async"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#11",
        axutils_features: &["tempfile"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#12",
        axutils_features: &["tempfile"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#13",
        axutils_features: &["tempfile-async"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#14",
        axutils_features: &["tempfile-async"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#15",
        axutils_features: &["tempfile", "tempfile-async"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#16",
        axutils_features: &["tempfile", "tempfile-async"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#17",
        axutils_features: &["tempfile", "tempfile-async"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#18",
        axutils_features: &["tempfile", "tempfile-async"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#19",
        axutils_features: &["tempfile", "tempfile-async"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#20",
        axutils_features: &["tempfile"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#21",
        axutils_features: &["tempfile"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#22",
        axutils_features: &["tempfile"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#23",
        axutils_features: &["tempfile"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#24",
        axutils_features: &["tempfile"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#25",
        axutils_features: &["tempfile"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#26",
        axutils_features: &["tempfile"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#27",
        axutils_features: &["tempfile"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#28",
        axutils_features: &["tempfile"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#29",
        axutils_features: &["tempfile"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#30",
        axutils_features: &["tempfile-async"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#31",
        axutils_features: &["tempfile-async"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#32",
        axutils_features: &["tempfile-async"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#33",
        axutils_features: &["tempfile-async"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#34",
        axutils_features: &["tempfile-async"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#35",
        axutils_features: &["tempfile-async"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#36",
        axutils_features: &["tempfile-async"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#37",
        axutils_features: &["tempfile-async"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#38",
        axutils_features: &["tempfile-async"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#39",
        axutils_features: &["tempfile-async"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#40",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#41",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#42",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#43",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#44",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#45",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#46",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#47",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#48",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#49",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#50",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#51",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#52",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#53",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#54",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#55",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#56",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#57",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#58",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#59",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#60",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#61",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#62",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#63",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#64",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#65",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#66",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#67",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#68",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#69",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#70",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#71",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#72",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#73",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#74",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/fs.md#75",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#1",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#2",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#3",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#4",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#5",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#6",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#7",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#8",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#9",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#10",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#11",
        axutils_features: F_SQLX,
        direct_dependencies: &[DEP_SQLX],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#12",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#13",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#14",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#15",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#16",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#17",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#18",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#19",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#20",
        axutils_features: F_SQLX,
        direct_dependencies: &[DEP_SQLX],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#21",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#22",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#23",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#24",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#25",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#26",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#27",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#28",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#29",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#30",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#31",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#32",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#33",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#34",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#35",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#36",
        axutils_features: F_SQLX,
        direct_dependencies: &[DEP_SQLX],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/sqlx.md#37",
        axutils_features: F_SQLX,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/path.md#1",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/path.md#2",
        axutils_features: F_NONE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/path.md#3",
        axutils_features: F_NONE,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/path.md#4",
        axutils_features: F_NONE,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/path.md#5",
        axutils_features: F_NONE,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/path.md#6",
        axutils_features: F_NONE,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/path.md#7",
        axutils_features: F_NONE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/path.md#8",
        axutils_features: F_NONE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/path.md#9",
        axutils_features: F_NONE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#1",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#2",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#3",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#4",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#5",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#6",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#7",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#8",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#9",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#10",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#11",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#12",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#13",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#14",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#15",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#16",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#17",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#18",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#19",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#20",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#21",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#22",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#23",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#24",
        axutils_features: &["tokio"],
        direct_dependencies: &[DEP_TIME],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#25",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#26",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#27",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#28",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#29",
        axutils_features: &["tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#30",
        axutils_features: &["tokio", "tokio-util"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#31",
        axutils_features: &["tokio", "tokio-util"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#32",
        axutils_features: &["tokio", "tokio-util"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#33",
        axutils_features: &["tokio", "tokio-util"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#34",
        axutils_features: &["tokio", "tokio-util"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#35",
        axutils_features: &["tokio", "tokio-util"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#36",
        axutils_features: &["tokio", "tokio-util"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#37",
        axutils_features: &["tokio", "tokio-util"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#38",
        axutils_features: &["tokio", "tokio-util"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#39",
        axutils_features: &["tokio", "tokio-util"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#40",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/tokio.md#41",
        axutils_features: &["tokio"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/random.md#1",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/random.md#2",
        axutils_features: F_RANDOM,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/random.md#3",
        axutils_features: F_RANDOM,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/random.md#4",
        axutils_features: F_RANDOM,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/random.md#5",
        axutils_features: F_RANDOM,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/random.md#6",
        axutils_features: F_RANDOM,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/random.md#7",
        axutils_features: F_RANDOM,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/random.md#8",
        axutils_features: F_RANDOM,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/random.md#9",
        axutils_features: F_RANDOM,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/random.md#10",
        axutils_features: F_RANDOM,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#1",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/config.md#2",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/config.md#3",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#4",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#5",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#6",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#7",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#8",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#9",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#10",
        axutils_features: &["serde", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#11",
        axutils_features: &["serde"],
        direct_dependencies: &[DEP_SERDE],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#12",
        axutils_features: &["serde", "tokio"],
        direct_dependencies: &[DEP_SERDE],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#13",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#14",
        axutils_features: &["serde"],
        direct_dependencies: &[DEP_SERDE],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#15",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#16",
        axutils_features: &["serde"],
        direct_dependencies: &[DEP_SERDE],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#17",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#18",
        axutils_features: &["serde"],
        direct_dependencies: &[DEP_SERDE],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#19",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#20",
        axutils_features: &["serde"],
        direct_dependencies: &[DEP_SERDE],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#21",
        axutils_features: &["serde", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#22",
        axutils_features: &["serde", "tokio"],
        direct_dependencies: &[DEP_SERDE],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#23",
        axutils_features: &["serde", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#24",
        axutils_features: &["serde", "tokio"],
        direct_dependencies: &[DEP_SERDE],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#25",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#26",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#27",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#28",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#29",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#30",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#31",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#32",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#33",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#34",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#35",
        axutils_features: &["serde", "toml"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#36",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#37",
        axutils_features: &["serde", "serde-saphyr"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#38",
        axutils_features: &["serde"],
        direct_dependencies: &[DEP_SERDE],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#39",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#40",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#41",
        axutils_features: &["serde", "serde-saphyr"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#42",
        axutils_features: &["serde", "toml"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#43",
        axutils_features: &["serde", "rust-ini"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/config.md#44",
        axutils_features: &["serde"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#1",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#2",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#3",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#4",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#5",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#6",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#7",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#8",
        axutils_features: &["encoding_rs"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#9",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#10",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#11",
        axutils_features: &["encoding_rs"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#12",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#13",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#14",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#15",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#16",
        axutils_features: &["base64"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#17",
        axutils_features: &["base64"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#18",
        axutils_features: &["base64"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#19",
        axutils_features: &["base64"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#20",
        axutils_features: &["base64"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#21",
        axutils_features: &["base64"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#22",
        axutils_features: &["base64"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#23",
        axutils_features: &["md5"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#24",
        axutils_features: &["md5"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#25",
        axutils_features: &["md5"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#26",
        axutils_features: &["md5"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#27",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#28",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#29",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#30",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#31",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#32",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#33",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#34",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#35",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#36",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#37",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#38",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#39",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#40",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#41",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#42",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#43",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#44",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#45",
        axutils_features: &["base64", "aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#46",
        axutils_features: &["base64", "aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#47",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#48",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#49",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#50",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#51",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#52",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#53",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#54",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#55",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#56",
        axutils_features: &["aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#57",
        axutils_features: &["base64", "aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/crypto.md#58",
        axutils_features: &["base64", "aes"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#1",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/redis.md#2",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/redis.md#3",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#4",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#5",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#6",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#7",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#8",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#9",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#10",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#11",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#12",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#13",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#14",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#15",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#16",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#17",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#18",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#19",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#20",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#21",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#22",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#23",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#24",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#25",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#26",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#27",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#28",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#29",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#30",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#31",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#32",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#33",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#34",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#35",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#36",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#37",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#38",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#39",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#40",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#41",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#42",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#43",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#44",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#45",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#46",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#47",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#48",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#49",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#50",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#51",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#52",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#53",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#54",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#55",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#56",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#57",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#58",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#59",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#60",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#61",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#62",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#63",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#64",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#65",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#66",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#67",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#68",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#69",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#70",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#71",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#72",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#73",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#74",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#75",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#76",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#77",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#78",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#79",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#80",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#81",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#82",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#83",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#84",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#85",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#86",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#87",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#88",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#89",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#90",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#91",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#92",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#93",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#94",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#95",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#96",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#97",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#98",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#99",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#100",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#101",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#102",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#103",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#104",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#105",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#106",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#107",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#108",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#109",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#110",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#111",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#112",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#113",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#114",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#115",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#116",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#117",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#118",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#119",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#120",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#121",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#122",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#123",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#124",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#125",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#126",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#127",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#128",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#129",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#130",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#131",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#132",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#133",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#134",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#135",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#136",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#137",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#138",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#139",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#140",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#141",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#142",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#143",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#144",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#145",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#146",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#147",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#148",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#149",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#150",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#151",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#152",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#153",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#154",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#155",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#156",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#157",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#158",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#159",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#160",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#161",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#162",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#163",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#164",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#165",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#166",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#167",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#168",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#169",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#170",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#171",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#172",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#173",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#174",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#175",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#176",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#177",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#178",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#179",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#180",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#181",
        axutils_features: &["redis"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#182",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#183",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#184",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#185",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#186",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#187",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#188",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#189",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#190",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#191",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#192",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#193",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#194",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#195",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#196",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#197",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#198",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#199",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#200",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#201",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#202",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#203",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#204",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#205",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#206",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#207",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#208",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#209",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#210",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#211",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#212",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#213",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#214",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#215",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#216",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#217",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#218",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#219",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#220",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#221",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#222",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#223",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#224",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#225",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#226",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#227",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#228",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#229",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/redis.md#230",
        axutils_features: &["redis", "tokio"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#1",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/http.md#2",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/http.md#3",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/http.md#4",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/http.md#5",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#6",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#7",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#8",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#9",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#10",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#11",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#12",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#13",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#14",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#15",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#16",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#17",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#18",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#19",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#20",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#21",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#22",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#23",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#24",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#25",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#26",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#27",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#28",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#29",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#30",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#31",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#32",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#33",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#34",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#35",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#36",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#37",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#38",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#39",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#40",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#41",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#42",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#43",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#44",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#45",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#46",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#47",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#48",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#49",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#50",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#51",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#52",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#53",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#54",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#55",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#56",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#57",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#58",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#59",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#60",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#61",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#62",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#63",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#64",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#65",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#66",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#67",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#68",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#69",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#70",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#71",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#72",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#73",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#74",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#75",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#76",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#77",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#78",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#79",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#80",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#81",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#82",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#83",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#84",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#85",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#86",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#87",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#88",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#89",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#90",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#91",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#92",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#93",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#94",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#95",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#96",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#97",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#98",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#99",
        axutils_features: &["http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#100",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#101",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#102",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#103",
        axutils_features: &["http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#104",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#105",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#106",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#107",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#108",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#109",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#110",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#111",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#112",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#113",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#114",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#115",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#116",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#117",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#118",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#119",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#120",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#121",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#122",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#123",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#124",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#125",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#126",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#127",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#128",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#129",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#130",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#131",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#132",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#133",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#134",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#135",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#136",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#137",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#138",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#139",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#140",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#141",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#142",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#143",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#144",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#145",
        axutils_features: &["serde", "http"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#146",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#147",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#148",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#149",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#150",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#151",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#152",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#153",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#154",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#155",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#156",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#157",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#158",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#159",
        axutils_features: &["serde", "http", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#160",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#161",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#162",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#163",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#164",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#165",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#166",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#167",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#168",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#169",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/http.md#170",
        axutils_features: &["http"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#1",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/convert.md#2",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#3",
        axutils_features: &["itoa"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#4",
        axutils_features: &["itoa"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#5",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/convert.md#6",
        axutils_features: &["itoa"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#7",
        axutils_features: &["itoa"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#8",
        axutils_features: &["itoa"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#9",
        axutils_features: &["itoa"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#10",
        axutils_features: &["itoa"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#11",
        axutils_features: &["ryu"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#12",
        axutils_features: &["ryu"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#13",
        axutils_features: &["ryu"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#14",
        axutils_features: &["ryu"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#15",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/convert.md#16",
        axutils_features: &["ryu"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#17",
        axutils_features: &["ryu"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#18",
        axutils_features: &["ryu"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#19",
        axutils_features: &["ryu"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#20",
        axutils_features: &["ryu"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#21",
        axutils_features: &["uuid"],
        direct_dependencies: &[DEP_UUID],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#22",
        axutils_features: &["uuid"],
        direct_dependencies: &[DEP_UUID],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#23",
        axutils_features: &["uuid"],
        direct_dependencies: &[DEP_UUID],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#24",
        axutils_features: &["uuid"],
        direct_dependencies: &[DEP_UUID],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#25",
        axutils_features: &["uuid"],
        direct_dependencies: &[DEP_UUID],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/convert.md#26",
        axutils_features: &["uuid"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#1",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/time.md#2",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/time.md#3",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#4",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#5",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#6",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#7",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#8",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#9",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#10",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#11",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#12",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#13",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#14",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#15",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#16",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#17",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#18",
        axutils_features: &["chrono"],
        direct_dependencies: &[DEP_CHRONO],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#19",
        axutils_features: &["chrono"],
        direct_dependencies: &[DEP_CHRONO],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#20",
        axutils_features: &["chrono"],
        direct_dependencies: &[DEP_CHRONO],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#21",
        axutils_features: &["chrono"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#22",
        axutils_features: &["chrono"],
        direct_dependencies: &[DEP_CHRONO],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#23",
        axutils_features: &["chrono"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#24",
        axutils_features: &["time"],
        direct_dependencies: &[DEP_TIME],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#25",
        axutils_features: &["time"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#26",
        axutils_features: &["time"],
        direct_dependencies: &[DEP_TIME],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#27",
        axutils_features: &["time"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#28",
        axutils_features: &["time"],
        direct_dependencies: &[DEP_TIME],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#29",
        axutils_features: &["time"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#30",
        axutils_features: &["jiff"],
        direct_dependencies: &[DEP_JIFF],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#31",
        axutils_features: &["jiff"],
        direct_dependencies: &[DEP_JIFF],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#32",
        axutils_features: &["jiff"],
        direct_dependencies: &[DEP_JIFF],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#33",
        axutils_features: &["jiff"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#34",
        axutils_features: &["jiff"],
        direct_dependencies: &[DEP_JIFF],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#35",
        axutils_features: &["jiff"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#36",
        axutils_features: &["chrono"],
        direct_dependencies: &[DEP_CHRONO],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#37",
        axutils_features: &["chrono"],
        direct_dependencies: &[DEP_CHRONO],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#38",
        axutils_features: &["chrono"],
        direct_dependencies: &[DEP_CHRONO],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#39",
        axutils_features: &["chrono"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#40",
        axutils_features: &["chrono"],
        direct_dependencies: &[DEP_CHRONO],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/time.md#41",
        axutils_features: &["chrono"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/log.md#1",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/log.md#2",
        axutils_features: F_LOG,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/log.md#3",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/log.md#4",
        axutils_features: F_LOG,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/log.md#5",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/log.md#6",
        axutils_features: F_LOG,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/log.md#7",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/log.md#8",
        axutils_features: F_LOG,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/log.md#9",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/log.md#10",
        axutils_features: F_LOG,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/log.md#11",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/log.md#12",
        axutils_features: F_LOG,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/log.md#13",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/log.md#14",
        axutils_features: F_LOG,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/log.md#15",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/log.md#16",
        axutils_features: F_LOG,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/log.md#17",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/log.md#18",
        axutils_features: F_LOG,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/log.md#19",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/log.md#20",
        axutils_features: F_LOG,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/log.md#21",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/log.md#22",
        axutils_features: F_LOG,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/log.md#23",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/log.md#24",
        axutils_features: F_LOG,
        direct_dependencies: &[DEP_SQLX],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/log.md#25",
        axutils_features: F_LOG,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/log.md#26",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/log.md#27",
        axutils_features: F_LOG,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/log.md#28",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/log.md#29",
        axutils_features: F_LOG,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/log.md#30",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/log.md#31",
        axutils_features: F_LOG,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#1",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/axum.md#2",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[DEP_AXUM],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#3",
        axutils_features: F_AXUM_HTTP,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#4",
        axutils_features: F_AXUM_HTTP,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#5",
        axutils_features: F_AXUM_HTTP,
        direct_dependencies: &[DEP_AXUM],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#6",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[DEP_AXUM],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#7",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#8",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#9",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[DEP_AXUM],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#10",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[DEP_AXUM],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#11",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[DEP_AXUM, DEP_TOWER],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#12",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[DEP_AXUM],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#13",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[DEP_AXUM],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#14",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#15",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[DEP_AXUM],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#16",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#17",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#18",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#19",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#20",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#21",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#22",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#23",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#24",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#25",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#26",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#27",
        axutils_features: F_AXUM_HTTP,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#28",
        axutils_features: F_AXUM_HTTP,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#29",
        axutils_features: F_AXUM_GOVERNOR,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#30",
        axutils_features: F_AXUM_GOVERNOR,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#31",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#32",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#33",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#34",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#35",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#36",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#37",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#38",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[DEP_AXUM],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#39",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#40",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#41",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#42",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#43",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#44",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#45",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#46",
        axutils_features: F_AXUM_BASE,
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#47",
        axutils_features: F_AXUM_TOWER,
        direct_dependencies: &[DEP_TOWER],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#48",
        axutils_features: F_AXUM_TOWER,
        direct_dependencies: &[DEP_TOWER],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#49",
        axutils_features: F_AXUM_TOWER,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#50",
        axutils_features: F_AXUM_HTTP,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#51",
        axutils_features: F_AXUM_HTTP,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#52",
        axutils_features: F_AXUM_HTTP,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/axum.md#53",
        axutils_features: F_AXUM_TRACE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#1",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#2",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#3",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#4",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#5",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#6",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#7",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#8",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#9",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#10",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#11",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#12",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#13",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#14",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#15",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#16",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#17",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#18",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#19",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#20",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#21",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#22",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#23",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#24",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#25",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#26",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#27",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#28",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#29",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#30",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#31",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#32",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#33",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#34",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#35",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#36",
        axutils_features: &["jwt"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#37",
        axutils_features: &["jwt"],
        direct_dependencies: &[DEP_SERDE],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#38",
        axutils_features: &["jwt"],
        direct_dependencies: &[DEP_SERDE],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/jwt.md#39",
        axutils_features: &["jwt"],
        direct_dependencies: &[DEP_SERDE],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#1",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#2",
        axutils_features: F_SCHEDULER,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#3",
        axutils_features: F_SCHEDULER,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#4",
        axutils_features: F_SCHEDULER,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#5",
        axutils_features: F_SCHEDULER,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#6",
        axutils_features: F_SCHEDULER,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#7",
        axutils_features: F_SCHEDULER,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#8",
        axutils_features: F_SCHEDULER,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#9",
        axutils_features: F_SCHEDULER,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#10",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#11",
        axutils_features: F_SCHEDULER,
        direct_dependencies: &[DEP_TIME],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#12",
        axutils_features: F_SCHEDULER,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#13",
        axutils_features: F_SCHEDULER,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#14",
        axutils_features: F_SCHEDULER,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#15",
        axutils_features: F_SCHEDULER,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#16",
        axutils_features: F_SCHEDULER,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#17",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#18",
        axutils_features: F_SCHEDULER,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#19",
        axutils_features: F_SCHEDULER,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#20",
        axutils_features: F_SCHEDULER,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#21",
        axutils_features: F_SCHEDULER,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/scheduler.md#22",
        axutils_features: F_SCHEDULER,
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/allocator.md#1",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/allocator.md#2",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/allocator.md#3",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/allocator.md#4",
        axutils_features: F_NONE,
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/allocator.md#5",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/format.md#1",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/format.md#2",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/format.md#3",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/format.md#4",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/format.md#5",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/format.md#6",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/format.md#7",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/format.md#8",
        axutils_features: &["serde", "strfmt"],
        direct_dependencies: &[DEP_SERDE],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/format.md#9",
        axutils_features: &["serde", "minijinja"],
        direct_dependencies: &[DEP_SERDE],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/format.md#10",
        axutils_features: &["serde", "minijinja"],
        direct_dependencies: &[DEP_SERDE],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/reg.md#1",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/reg.md#2",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/reg.md#3",
        axutils_features: &["regex"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/reg.md#4",
        axutils_features: &["regex"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/reg.md#5",
        axutils_features: &["regex"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/reg.md#6",
        axutils_features: &["regex"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/reg.md#7",
        axutils_features: &["regex", "libphonenumber"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/email.md#1",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/email.md#2",
        axutils_features: &[],
        direct_dependencies: &[],
        mode: CompileMode::ExplicitlyExcluded,
        exclusion_reason: Some(
            "非 Rust fence（配置、签名、文本或命令），按计划显式排除；不作为 Rust crate 编译。",
        ),
    },
    BlockMetadata {
        key: "docs/examples/email.md#3",
        axutils_features: &["lettre"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/email.md#4",
        axutils_features: &["lettre"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/email.md#5",
        axutils_features: &["lettre"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/email.md#6",
        axutils_features: &["lettre"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/email.md#7",
        axutils_features: &["lettre"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/email.md#8",
        axutils_features: &["lettre"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/email.md#9",
        axutils_features: &["lettre"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/email.md#10",
        axutils_features: &["lettre"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/email.md#11",
        axutils_features: &["lettre"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/email.md#12",
        axutils_features: &["lettre"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/email.md#13",
        axutils_features: &["lettre", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/email.md#14",
        axutils_features: &["lettre"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/email.md#15",
        axutils_features: &["lettre"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/email.md#16",
        axutils_features: &["lettre"],
        direct_dependencies: &[],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/email.md#17",
        axutils_features: &["lettre", "tokio"],
        direct_dependencies: &[DEP_TOKIO],
        mode: CompileMode::NoRun,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/email.md#18",
        axutils_features: &["lettre"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
    BlockMetadata {
        key: "docs/examples/email.md#19",
        axutils_features: &["lettre"],
        direct_dependencies: &[],
        mode: CompileMode::Compiled,
        exclusion_reason: None,
    },
];

#[test]
fn docs_examples_are_complete() {
    let documents =
        load_documents().unwrap_or_else(|error| panic!("failed to enumerate docs: {error}"));
    validate_metadata(&documents);

    let fence_count: usize = documents.iter().map(|document| document.fences.len()).sum();
    println!(
        "docs_examples enumeration: {} documents, {fence_count} fenced blocks",
        documents.len()
    );
    for document in &documents {
        println!("  {}: {} blocks", document.path, document.fences.len());
    }
}

#[test]
#[ignore = "逐块派生临时 crate 并 cargo check --offline；局部验证用 AXUTILS_DOCS_EXAMPLE_FILTER 按文档过滤"]
fn compile_docs_examples_offline() {
    let documents =
        load_documents().unwrap_or_else(|error| panic!("failed to enumerate docs: {error}"));
    validate_metadata(&documents);

    let workspace = TempDir::new("axutils-docs-examples").unwrap_or_else(|error| {
        panic!("failed to create isolated docs-example workspace: {error}")
    });
    let target_dir = workspace.path().join("target");
    fs::create_dir_all(&target_dir)
        .unwrap_or_else(|error| panic!("failed to create isolated CARGO_TARGET_DIR: {error}"));

    let mut counts = [0usize; 3];
    let mut case_number = 0usize;
    let filter = env::var("AXUTILS_DOCS_EXAMPLE_FILTER").ok();
    let start = env::var("AXUTILS_DOCS_EXAMPLE_START").ok();
    let mut started = start.is_none();
    for document in &documents {
        for (index, fence) in document.fences.iter().enumerate() {
            let block_number = index + 1;
            let key = format!("{}#{block_number}", document.path);
            if !started {
                if start.as_deref() == Some(key.as_str()) {
                    started = true;
                } else {
                    continue;
                }
            }
            if filter.as_deref().is_some_and(|value| !key.contains(value)) {
                continue;
            }
            let metadata = metadata_for(&key).expect("metadata was validated above");
            report_block(document, block_number, metadata);

            match metadata.mode {
                CompileMode::ExplicitlyExcluded => {
                    counts[2] += 1;
                    continue;
                }
                CompileMode::Compiled => counts[0] += 1,
                CompileMode::NoRun => counts[1] += 1,
            }

            case_number += 1;
            let case_dir = workspace.path().join(format!("case-{case_number:04}"));
            fs::create_dir_all(case_dir.join("src"))
                .unwrap_or_else(|error| panic!("{key}: failed to create temporary crate: {error}"));
            let manifest = case_dir.join("Cargo.toml");
            let source = case_dir.join("src/main.rs");
            write_case(&manifest, &source, fence, metadata)
                .unwrap_or_else(|error| panic!("{key}: failed to write temporary crate: {error}"));

            let result = run_cargo_check(&manifest, &target_dir, metadata, &key);
            let _ = fs::remove_dir_all(&case_dir);
            if let Err(error) = result {
                panic!("{key} failed: {error}");
            }

            for additional in ADDITIONAL_COMPILE_COMBINATIONS
                .iter()
                .filter(|additional| additional.key == key)
            {
                report_block(document, block_number, additional);
                match additional.mode {
                    CompileMode::Compiled => counts[0] += 1,
                    CompileMode::NoRun => counts[1] += 1,
                    CompileMode::ExplicitlyExcluded => {
                        unreachable!("additional combinations are always compiled")
                    }
                }
                case_number += 1;
                let case_dir = workspace.path().join(format!("case-{case_number:04}"));
                fs::create_dir_all(case_dir.join("src")).unwrap_or_else(|error| {
                    panic!("{key}: failed to create additional temporary crate: {error}")
                });
                let manifest = case_dir.join("Cargo.toml");
                let source = case_dir.join("src/main.rs");
                write_case(&manifest, &source, fence, additional).unwrap_or_else(|error| {
                    panic!("{key}: failed to write additional temporary crate: {error}")
                });
                let result = run_cargo_check(&manifest, &target_dir, additional, &key);
                let _ = fs::remove_dir_all(&case_dir);
                if let Err(error) = result {
                    panic!("{key} additional combination failed: {error}");
                }
            }
        }
    }
    assert!(
        started,
        "AXUTILS_DOCS_EXAMPLE_START did not match any block: {:?}",
        start
    );

    println!(
        "docs_examples summary: compiled={}, no_run={}, explicitly_excluded={}",
        counts[0], counts[1], counts[2]
    );
}

fn load_documents() -> io::Result<Vec<Document>> {
    let mut paths = Vec::new();
    for line in MODULE_MAP.lines() {
        let Some(start) = line.find("docs/examples/") else {
            continue;
        };
        let suffix = &line[start..];
        let end = suffix
            .char_indices()
            .find(|(_, character)| {
                !character.is_ascii_alphanumeric()
                    && *character != '_'
                    && *character != '-'
                    && *character != '.'
                    && *character != '/'
            })
            .map_or(suffix.len(), |(index, _)| index);
        let path = &suffix[..end];
        if !path.ends_with(".md") || paths.iter().any(|known| known == path) {
            continue;
        }
        paths.push(path.to_owned());
    }

    if paths.len() != 19 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "module-map yielded {} unique docs/examples paths, expected 19",
                paths.len()
            ),
        ));
    }

    let mut documents = Vec::with_capacity(paths.len());
    for path in paths {
        let absolute = Path::new(ROOT_MANIFEST).join(&path);
        let source = fs::read_to_string(&absolute)?;
        documents.push(Document {
            path,
            fences: parse_fences(&source),
        });
    }
    Ok(documents)
}

fn parse_fences(source: &str) -> Vec<Fence> {
    let lines: Vec<&str> = source.lines().collect();
    let mut fences = Vec::new();
    let mut open: Option<(u8, usize, String, Vec<String>)> = None;

    for line in &lines {
        let line_without_cr = line.strip_suffix('\r').unwrap_or(line);
        let trimmed = line_without_cr.trim_start();
        let Some(marker) = trimmed.as_bytes().first().copied() else {
            if let Some((_, _, _, body)) = open.as_mut() {
                body.push((*line).to_owned());
            }
            continue;
        };
        if marker != b'`' && marker != b'~' {
            if let Some((_, _, _, body)) = open.as_mut() {
                body.push((*line).to_owned());
            }
            continue;
        }
        let marker_len = trimmed.bytes().take_while(|byte| *byte == marker).count();
        if marker_len < 3 {
            if let Some((_, _, _, body)) = open.as_mut() {
                body.push((*line).to_owned());
            }
            continue;
        }

        match open.as_mut() {
            Some((open_marker, open_len, _, body))
                if *open_marker == marker
                    && marker_len >= *open_len
                    && trimmed[marker_len..].trim().is_empty() =>
            {
                let (open_marker, open_len, info, body) = open.take().expect("open fence exists");
                debug_assert_eq!(open_marker, marker);
                debug_assert!(marker_len >= open_len);
                fences.push(Fence {
                    info,
                    body: body.join("\n"),
                });
            }
            Some((_, _, _, body)) => body.push((*line).to_owned()),
            None => {
                let info = trimmed[marker_len..].trim().to_owned();
                open = Some((marker, marker_len, info, Vec::new()));
            }
        }
    }

    fences
}

fn validate_metadata(documents: &[Document]) {
    let expected: BTreeSet<&str> = documents
        .iter()
        .flat_map(|document| {
            document
                .fences
                .iter()
                .enumerate()
                .map(move |(index, _)| format!("{}#{}", document.path, index + 1))
        })
        .map(|key| Box::leak(key.into_boxed_str()) as &str)
        .collect();
    let actual: BTreeSet<&str> = BLOCK_METADATA.iter().map(|metadata| metadata.key).collect();

    if expected != actual {
        let missing: Vec<_> = expected.difference(&actual).copied().collect();
        let extra: Vec<_> = actual.difference(&expected).copied().collect();
        panic!("metadata/document mismatch; missing={missing:?}, extra={extra:?}");
    }

    let mut seen = BTreeSet::new();
    for metadata in BLOCK_METADATA {
        assert!(
            seen.insert(metadata.key),
            "duplicate metadata key: {}",
            metadata.key
        );
        for feature in metadata.axutils_features {
            assert!(
                VALID_AXUTILS_FEATURES.contains(feature),
                "{} lists unknown axutils feature `{feature}`",
                metadata.key
            );
        }
        if metadata.mode == CompileMode::ExplicitlyExcluded {
            assert!(
                metadata
                    .exclusion_reason
                    .is_some_and(|reason| !reason.trim().is_empty()),
                "{} is excluded without a reason",
                metadata.key
            );
        } else {
            assert!(
                metadata.exclusion_reason.is_none(),
                "{} has an exclusion reason but is compiled",
                metadata.key
            );
        }
    }

    let mut additional_seen = BTreeSet::new();
    for metadata in ADDITIONAL_COMPILE_COMBINATIONS {
        assert!(
            expected.contains(metadata.key),
            "additional combination references unknown block: {}",
            metadata.key
        );
        let primary = metadata_for(metadata.key).expect("primary metadata was checked above");
        assert_ne!(
            primary.mode,
            CompileMode::ExplicitlyExcluded,
            "additional combination references excluded block: {}",
            metadata.key
        );
        assert_eq!(
            metadata.mode, primary.mode,
            "additional combination mode differs from primary block: {}",
            metadata.key
        );
        assert!(metadata.exclusion_reason.is_none());
        for feature in metadata.axutils_features {
            assert!(
                VALID_AXUTILS_FEATURES.contains(feature),
                "{} additional combination lists unknown feature `{feature}`",
                metadata.key
            );
        }
        let identity = format!("{}:{}", metadata.key, metadata.axutils_features.join(","));
        assert!(
            additional_seen.insert(identity),
            "duplicate additional combination for {}",
            metadata.key
        );
    }

    let mut mode_mismatches = Vec::new();
    for document in documents {
        for (index, fence) in document.fences.iter().enumerate() {
            let key = format!("{}#{}", document.path, index + 1);
            let metadata = metadata_for(&key).expect("metadata key was checked above");
            let language = fence_language(&fence.info);
            if language == Some("rust") {
                let expected_mode = if fence_has_flag(&fence.info, "no_run") {
                    CompileMode::NoRun
                } else {
                    CompileMode::Compiled
                };
                if metadata.mode != expected_mode {
                    mode_mismatches.push(format!(
                        "{key}: metadata={:?}, fence=`{}` expects {expected_mode:?}",
                        metadata.mode, fence.info
                    ));
                }
            } else {
                if metadata.mode != CompileMode::ExplicitlyExcluded {
                    mode_mismatches.push(format!(
                        "{key}: metadata={:?}, non-rust fence=`{}` expects ExplicitlyExcluded",
                        metadata.mode, fence.info
                    ));
                }
            }
        }
    }
    assert!(
        mode_mismatches.is_empty(),
        "metadata fence mode mismatches:\n{}",
        mode_mismatches.join("\n")
    );
}

fn metadata_for(key: &str) -> Option<&'static BlockMetadata> {
    BLOCK_METADATA.iter().find(|metadata| metadata.key == key)
}

fn fence_language(info: &str) -> Option<&str> {
    info.split(|character: char| character == ',' || character.is_ascii_whitespace())
        .find(|part| !part.is_empty())
}

fn fence_has_flag(info: &str, flag: &str) -> bool {
    info.split(|character: char| character == ',' || character.is_ascii_whitespace())
        .any(|part| part == flag)
}

fn report_block(document: &Document, block_number: usize, metadata: &BlockMetadata) {
    let features = if metadata.axutils_features.is_empty() {
        "-".to_owned()
    } else {
        metadata.axutils_features.join(",")
    };
    let dependencies = if metadata.direct_dependencies.is_empty() {
        "-".to_owned()
    } else {
        metadata
            .direct_dependencies
            .iter()
            .map(|dependency| dependency.name)
            .collect::<Vec<_>>()
            .join(",")
    };
    if let Some(reason) = metadata.exclusion_reason {
        println!(
            "{} #{} axutils=[{}] direct=[{}] status={} reason={reason}",
            document.path,
            block_number,
            features,
            dependencies,
            metadata.mode.as_str()
        );
    } else {
        println!(
            "{} #{} axutils=[{}] direct=[{}] status={}",
            document.path,
            block_number,
            features,
            dependencies,
            metadata.mode.as_str()
        );
    }
}

fn write_case(
    manifest: &Path,
    source_path: &Path,
    fence: &Fence,
    metadata: &BlockMetadata,
) -> io::Result<()> {
    let root_path = Path::new(ROOT_MANIFEST)
        .to_string_lossy()
        .replace('\\', "/");
    let mut manifest_text = String::new();
    manifest_text.push_str(
        "[package]\nname = \"axutils_docs_example\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n",
    );
    manifest_text.push_str("[features]\ndefault = []\n");
    for feature in VALID_AXUTILS_FEATURES {
        writeln!(manifest_text, "{feature} = []").expect("writing to String cannot fail");
    }
    manifest_text.push_str("\n[dependencies]\n");
    writeln!(
        manifest_text,
        "axutils = {{ path = \"{root_path}\", default-features = false, features = [{}] }}",
        quoted_list(metadata.axutils_features)
    )
    .expect("writing to String cannot fail");
    for dependency in metadata.direct_dependencies {
        writeln!(manifest_text, "{}", dependency_manifest_line(dependency))
            .expect("writing to String cannot fail");
    }
    fs::write(manifest, manifest_text)?;
    fs::write(source_path, wrap_rust_source(&fence.body))
}

fn dependency_manifest_line(dependency: &DirectDependency) -> String {
    if dependency.package.is_none() && dependency.default_features && dependency.features.is_empty()
    {
        return format!("{} = \"{}\"", dependency.name, dependency.version);
    }

    let mut line = format!(
        "{} = {{ version = \"{}\"",
        dependency.name, dependency.version
    );
    if let Some(package) = dependency.package {
        write!(line, ", package = \"{package}\"").expect("writing to String cannot fail");
    }
    if !dependency.default_features {
        line.push_str(", default-features = false");
    }
    if !dependency.features.is_empty() {
        write!(line, ", features = [{}]", quoted_list(dependency.features))
            .expect("writing to String cannot fail");
    }
    line.push_str(" }");
    line
}

fn quoted_list(items: &[&str]) -> String {
    items
        .iter()
        .map(|item| format!("\"{item}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

fn wrap_rust_source(body: &str) -> String {
    let mut crate_attributes = Vec::new();
    let mut code = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed == "#" {
            continue;
        }
        if let Some(hidden) = trimmed.strip_prefix("# ") {
            code.push(hidden.to_owned());
        } else if trimmed.starts_with("#![") {
            crate_attributes.push(line.to_owned());
        } else {
            code.push(line.to_owned());
        }
    }

    let code = code.join("\n");
    let mut source = String::new();
    for attribute in crate_attributes {
        source.push_str(&attribute);
        source.push('\n');
    }
    if contains_main_function(&code) {
        source.push_str(&code);
        source.push('\n');
        return source;
    }

    let returns_result = code.contains('?');
    if returns_result {
        source.push_str("fn main() -> Result<(), Box<dyn std::error::Error>> {\n");
    } else {
        source.push_str("fn main() {\n");
    }
    source.push_str(&code);
    source.push('\n');
    if returns_result {
        if !code.trim_end().ends_with(';') && !code.trim_end().ends_with('}') {
            source.push_str(";\n");
        }
        source.push_str("Ok(())\n");
    }
    source.push_str("}\n");
    source
}

fn contains_main_function(code: &str) -> bool {
    code.lines().any(|line| {
        let trimmed = line.trim_start();
        (trimmed.starts_with("fn main") || trimmed.starts_with("async fn main"))
            && trimmed.contains('(')
    })
}

fn run_cargo_check(
    manifest: &Path,
    target_dir: &Path,
    metadata: &BlockMetadata,
    key: &str,
) -> Result<(), String> {
    let output = Command::new("cargo")
        .arg("check")
        .arg("--offline")
        .arg("--manifest-path")
        .arg(manifest)
        .arg("--no-default-features")
        .arg("--features")
        .arg(metadata.axutils_features.join(","))
        .env("CARGO_TARGET_DIR", target_dir)
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .map_err(|error| format!("failed to start cargo: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let mut diagnostic = String::new();
    diagnostic.push_str(&String::from_utf8_lossy(&output.stdout));
    diagnostic.push_str(&String::from_utf8_lossy(&output.stderr));
    Err(format!(
        "cargo check exit={} for {}:\n{}",
        output.status,
        key,
        redact_diagnostic(&diagnostic, &[manifest, target_dir])
    ))
}

fn redact_diagnostic(diagnostic: &str, paths: &[&Path]) -> String {
    let mut redacted = diagnostic.to_owned();
    for path in paths {
        let path = path.to_string_lossy();
        redacted = redacted.replace(path.as_ref(), "<temporary-path>");
    }
    let mut lines = Vec::new();
    for line in redacted.lines().take(120) {
        let lower = line.to_ascii_lowercase();
        if lower.contains("password")
            || lower.contains("secret")
            || lower.contains("authorization")
            || lower.contains("cookie")
            || lower.contains("token")
        {
            lines.push("<redacted sensitive diagnostic line>".to_owned());
        } else {
            lines.push(line.to_owned());
        }
    }
    lines.join("\n")
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> io::Result<Self> {
        let base = env::temp_dir();
        for attempt in 0..100u32 {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = base.join(format!("{prefix}-{}-{nonce}-{attempt}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("failed to allocate unique temporary directory for {prefix}"),
        ))
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
