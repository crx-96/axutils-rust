//! Markdown 示例的离线编译闭环。
//!
//! 非 ignored 测试校验 module map、文档默认配置和 fence 覆盖的双向完整性。ignored 测试按
//! “axutils feature + 调用方直接依赖”完全相同的组合，将正向 Rust fence 合并为多 bin
//! 临时 crate；`compile_fail` fence 仍逐例隔离。局部验证可通过
//! `AXUTILS_DOCS_EXAMPLE_FILTER` 选择文档。

#[path = "docs_examples/parse.rs"]
mod parse;
#[path = "docs_examples/runner.rs"]
mod runner;

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
    "phone-validation",
    "template-strfmt",
    "template-minijinja",
    "chrono",
    "time",
    "jiff",
    "email",
    "email-async",
    "http",
    "http-async",
    "http-json",
    "redis",
    "redis-cluster",
    "redis-async",
    "redis-cluster-async",
    "sqlx",
    "sqlx-postgres",
    "sqlx-mysql",
    "sqlx-sqlite",
    "tokio",
    "task-group",
    "scheduler",
    "axum",
    "axum-tower",
    "axum-tower-http",
    "axum-governor",
    "fs-async",
    "fs-temp",
    "fs-temp-async",
    "config",
    "config-yaml",
    "config-toml",
    "config-ini",
    "config-async",
    "base64",
    "md5",
    "aes",
    "encoding_rs",
];

/// 重构前逐个正向 fence 执行 Cargo 的基线：870 个主 case，加 12 个时间后端 case。
const LEGACY_PER_BLOCK_CARGO_PROCESSES: usize = 882;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompileMode {
    Compiled,
    NoRun,
    CompileFail,
    ExplicitlyExcluded,
}

impl CompileMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Compiled => "compiled",
            Self::NoRun => "no_run",
            Self::CompileFail => "compile_fail",
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
struct CompileSettings {
    axutils_features: &'static [&'static str],
    direct_dependencies: &'static [DirectDependency],
}

#[derive(Clone, Copy, Debug)]
struct BlockOverride {
    fence_number: usize,
    settings: CompileSettings,
}

#[derive(Clone, Copy, Debug)]
struct DocumentMetadata {
    path: &'static str,
    defaults: CompileSettings,
    overrides: &'static [BlockOverride],
}

#[derive(Clone, Debug)]
struct BlockMetadata {
    key: String,
    axutils_features: &'static [&'static str],
    direct_dependencies: &'static [DirectDependency],
    mode: CompileMode,
    exclusion_reason: Option<&'static str>,
}

#[derive(Debug)]
struct Fence {
    info: String,
    body: String,
    start_line: usize,
}

#[derive(Debug)]
struct Document {
    path: String,
    fences: Vec<Fence>,
}

const DEP_CHRONO: DirectDependency = DirectDependency {
    name: "chrono",
    package: None,
    version: "0.4",
    default_features: false,
    features: &[],
};
const DEP_TIME: DirectDependency = DirectDependency {
    name: "time",
    package: None,
    version: "0.3",
    default_features: false,
    features: &[],
};
const DEP_JIFF: DirectDependency = DirectDependency {
    name: "jiff",
    package: None,
    version: "0.2",
    default_features: false,
    features: &[],
};
const DEP_SERDE: DirectDependency = DirectDependency {
    name: "serde",
    package: None,
    version: "1",
    default_features: true,
    features: &["derive"],
};
const DEP_TOKIO_MACROS: DirectDependency = DirectDependency {
    name: "tokio",
    package: None,
    version: "1",
    default_features: false,
    features: &["macros", "rt-multi-thread"],
};
const DEP_TOKIO_SCHEDULER: DirectDependency = DirectDependency {
    name: "tokio",
    package: None,
    version: "1",
    default_features: false,
    features: &["rt-multi-thread", "time"],
};
const DEP_TRACING: DirectDependency = DirectDependency {
    name: "tracing",
    package: None,
    version: "0.1",
    default_features: true,
    features: &[],
};
const DEP_UUID: DirectDependency = DirectDependency {
    name: "uuid",
    package: None,
    version: "1",
    default_features: false,
    features: &["std"],
};

const NO_DEPS: &[DirectDependency] = &[];
const CHRONO_DEP: &[DirectDependency] = &[DEP_CHRONO];
const TIME_DEP: &[DirectDependency] = &[DEP_TIME];
const JIFF_DEP: &[DirectDependency] = &[DEP_JIFF];
const SERDE_DEP: &[DirectDependency] = &[DEP_SERDE];
const TOKIO_MACROS_DEP: &[DirectDependency] = &[DEP_TOKIO_MACROS];
const TOKIO_SCHEDULER_DEP: &[DirectDependency] = &[DEP_TOKIO_SCHEDULER];
const TRACING_DEP: &[DirectDependency] = &[DEP_TRACING];
const UUID_DEP: &[DirectDependency] = &[DEP_UUID];
const HTTP_ASYNC_DEPS: &[DirectDependency] = &[DEP_SERDE, DEP_TOKIO_MACROS];

const fn settings(
    axutils_features: &'static [&'static str],
    direct_dependencies: &'static [DirectDependency],
) -> CompileSettings {
    CompileSettings {
        axutils_features,
        direct_dependencies,
    }
}

const fn block(fence_number: usize, settings: CompileSettings) -> BlockOverride {
    BlockOverride {
        fence_number,
        settings,
    }
}

const fn document(
    path: &'static str,
    defaults: CompileSettings,
    overrides: &'static [BlockOverride],
) -> DocumentMetadata {
    DocumentMetadata {
        path,
        defaults,
        overrides,
    }
}

// 每项先声明文档默认编译环境，紧邻的 override 只记录偏离默认值的 Rust fence。
const DOCUMENT_METADATA: &[DocumentMetadata] = &[
    document(
        "README.md",
        settings(&[], NO_DEPS),
        &[
            block(1, settings(&["config", "redis"], NO_DEPS)),
            block(5, settings(&["http"], NO_DEPS)),
            block(6, settings(&["jwt"], NO_DEPS)),
        ],
    ),
    document(
        "docs/module-map.md",
        settings(&["config", "redis"], NO_DEPS),
        &[],
    ),
    document(
        "docs/examples/fs.md",
        settings(&[], NO_DEPS),
        &[
            block(6, settings(&["fs-async"], NO_DEPS)),
            block(7, settings(&["fs-temp"], NO_DEPS)),
            block(8, settings(&["fs-temp"], NO_DEPS)),
            block(9, settings(&["fs-temp-async"], NO_DEPS)),
        ],
    ),
    document(
        "docs/examples/time.md",
        settings(&[], NO_DEPS),
        &[
            block(3, settings(&["chrono"], CHRONO_DEP)),
            block(4, settings(&["time"], TIME_DEP)),
            block(5, settings(&["jiff"], JIFF_DEP)),
        ],
    ),
    document(
        "docs/examples/crypto.md",
        settings(&[], NO_DEPS),
        &[
            block(3, settings(&["base64"], NO_DEPS)),
            block(4, settings(&["md5"], NO_DEPS)),
            block(5, settings(&["aes"], NO_DEPS)),
            block(6, settings(&["aes"], NO_DEPS)),
        ],
    ),
    document(
        "docs/examples/convert.md",
        settings(&["itoa"], NO_DEPS),
        &[
            block(3, settings(&["ryu"], NO_DEPS)),
            block(4, settings(&["uuid"], UUID_DEP)),
        ],
    ),
    document(
        "docs/examples/format.md",
        settings(&[], NO_DEPS),
        &[block(3, settings(&["template-minijinja"], SERDE_DEP))],
    ),
    document("docs/examples/path.md", settings(&[], NO_DEPS), &[]),
    document("docs/examples/random.md", settings(&["rand"], NO_DEPS), &[]),
    document(
        "docs/examples/reg.md",
        settings(&["regex"], NO_DEPS),
        &[block(3, settings(&["phone-validation"], NO_DEPS))],
    ),
    document("docs/examples/jwt.md", settings(&["jwt"], SERDE_DEP), &[]),
    document(
        "docs/examples/log.md",
        settings(&["logging"], TRACING_DEP),
        &[],
    ),
    document(
        "docs/examples/config.md",
        settings(&["config"], NO_DEPS),
        &[
            block(7, settings(&["config-toml"], NO_DEPS)),
            block(8, settings(&["config-async"], NO_DEPS)),
        ],
    ),
    document(
        "docs/examples/email.md",
        settings(&["email"], NO_DEPS),
        &[block(4, settings(&["email-async"], TOKIO_MACROS_DEP))],
    ),
    document(
        "docs/examples/http.md",
        settings(&["http"], NO_DEPS),
        &[block(
            5,
            settings(&["http-async", "http-json"], HTTP_ASYNC_DEPS),
        )],
    ),
    document(
        "docs/examples/redis.md",
        settings(&["redis"], NO_DEPS),
        &[
            block(4, settings(&["redis-cluster"], NO_DEPS)),
            block(8, settings(&["redis-async"], TOKIO_MACROS_DEP)),
            block(10, settings(&["redis-async"], TOKIO_MACROS_DEP)),
        ],
    ),
    document(
        "docs/examples/sqlx.md",
        settings(&["sqlx-postgres"], NO_DEPS),
        &[block(4, settings(&["sqlx-sqlite"], NO_DEPS))],
    ),
    document(
        "docs/examples/tokio.md",
        settings(&["tokio"], NO_DEPS),
        &[block(5, settings(&["task-group"], NO_DEPS))],
    ),
    document(
        "docs/examples/scheduler.md",
        settings(&["scheduler"], TOKIO_SCHEDULER_DEP),
        &[],
    ),
    document("docs/examples/axum.md", settings(&["axum"], NO_DEPS), &[]),
];

#[test]
fn docs_examples_are_complete() {
    let documents =
        parse::load_documents().unwrap_or_else(|error| panic!("failed to enumerate docs: {error}"));
    parse::validate_metadata(&documents);

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
#[ignore = "按 feature/直接依赖组合派生多 bin 临时 crate 并 cargo check --offline；局部验证用 AXUTILS_DOCS_EXAMPLE_FILTER"]
fn compile_docs_examples_offline() {
    let documents =
        parse::load_documents().unwrap_or_else(|error| panic!("failed to enumerate docs: {error}"));
    parse::validate_metadata(&documents);
    runner::compile_documents(&documents);
}
