use std::{
    collections::BTreeMap,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, OnceLock,
    },
};

#[derive(Clone, Copy)]
pub(super) struct FixtureCase {
    pub(super) feature: &'static str,
    pub(super) expected_success: bool,
    pub(super) diagnostics: &'static [ExpectedDiagnostic],
}

#[derive(Debug)]
pub(super) enum ExpectedDiagnostic {
    /// E0599 同时指出所属类型和缺失成员。
    Member(&'static str, &'static str),
    /// E0432 可在同一条主消息中逐项列出多个未解析的完整导入路径。
    Import(&'static str),
    /// 错误码、主消息指出的符号或约束，以及触发错误的 fixture 路径。
    AtPath(&'static str, &'static str, &'static str),
}

// Cargo 按 feature、profile 和包身份区分产物，并用构建锁协调共享目录的并发写入。
fn shared_build_target() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("feature-matrix")
}

pub(super) fn run_fixture_cases(name: &str, cases: &[FixtureCase]) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("semantic_feature_matrix")
        .join("Cargo.toml");
    let target = shared_build_target();
    let (fixture, copied_manifest) = copy_fixture_to_temporary_directory(&manifest);
    let source = fs::read_to_string(fixture.path().join("src/main.rs")).expect("read matrix probe");
    for case in cases {
        FIXTURE_CARGO_CALLS.fetch_add(1, Ordering::Relaxed);
        let output = run_fixture(&copied_manifest, &target, case.feature);
        assert_eq!(
            output.status.success(),
            case.expected_success,
            "fixture group {name}, feature {:?} unexpected status: {}\nstdout: {}\nstderr: {}",
            case.feature,
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        if !case.expected_success {
            let stderr =
                std::str::from_utf8(&output.stderr).expect("cargo diagnostics must be UTF-8");
            assert_expected_diagnostics(stderr, &source, case.feature, case.diagnostics);
        }
    }
    fixture.cleanup();
}

pub(super) fn run_downstream_contract(fixture_name: &str, subcommand: &str, feature_sets: &[&str]) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(fixture_name)
        .join("Cargo.toml");
    let target = shared_build_target();
    let (fixture, manifest) = copy_fixture_to_temporary_directory(&manifest);
    for &features in feature_sets {
        let mut command = Command::new("cargo");
        command
            .args([subcommand, "--offline", "--no-default-features", "-j", "2"])
            .arg("--manifest-path")
            .arg(&manifest)
            .arg("--target-dir")
            .arg(&target)
            .env("CARGO_TERM_COLOR", "never");
        if !features.is_empty() {
            command.arg("--features").arg(features);
        }
        FIXTURE_CARGO_CALLS.fetch_add(1, Ordering::Relaxed);
        if subcommand == "test" {
            command.args(["--", "--test-threads=1"]);
        }
        let output = command.output().expect("failed to run downstream fixture");
        assert!(
            output.status.success(),
            "fixture {fixture_name} failed for features {features:?}\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fixture.cleanup();
}

pub(super) fn assert_removed_provider_features(features: &[&str]) {
    let target = TemporaryTarget::new("axutils-removed-provider-features");
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .to_string_lossy()
        .replace('\\', "/");
    for feature in features {
        let probe = TemporaryTarget::new("axutils-removed-provider-probe");
        let manifest = probe.path().join("Cargo.toml");
        fs::write(
            &manifest,
            format!(
                "[package]\nname = \"axutils-removed-provider-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[features]\nprobe = [\"axutils/{feature}\"]\n\n[dependencies]\naxutils = {{ path = \"{repository}\", default-features = false }}\n"
            ),
        )
        .unwrap_or_else(|_| panic!("failed to write provider probe for {}", feature));
        fs::create_dir_all(probe.path().join("src"))
            .unwrap_or_else(|_| panic!("failed to create provider probe source for {}", feature));
        fs::write(probe.path().join("src/lib.rs"), "")
            .unwrap_or_else(|_| panic!("failed to write provider probe source for {}", feature));
        let output = Command::new("cargo")
            .arg("check")
            .arg("--manifest-path")
            .arg(&manifest)
            .arg("--target-dir")
            .arg(target.path())
            .arg("--no-default-features")
            .arg("--offline")
            .arg("--features")
            .arg("probe")
            .env("CARGO_TERM_COLOR", "never")
            .output()
            .unwrap_or_else(|_| panic!("failed to resolve removed provider feature {}", feature));
        assert!(
            !output.status.success(),
            "removed provider feature {} unexpectedly resolved",
            feature
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        let requested = format!("with feature `{feature}`");
        assert!(
            stderr.lines().any(|line| {
                line.contains(&requested) && line.contains("`axutils` does not have that feature")
            }),
            "removed provider feature {feature} failed for an unexpected reason: {stderr}"
        );
        probe.cleanup();
    }
    target.cleanup();
}

struct TemporaryTarget(PathBuf);

impl TemporaryTarget {
    fn new(prefix: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let parent = Path::new(env!("CARGO_TARGET_TMPDIR"));
        fs::create_dir_all(parent).unwrap_or_else(|error| {
            panic!(
                "failed to create matrix directory {}: {error}",
                parent.display()
            )
        });
        loop {
            let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!("{prefix}-{}-{counter}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => panic!(
                    "failed to create matrix directory {}: {error}",
                    path.display()
                ),
            }
        }
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn cleanup(self) {
        fs::remove_dir_all(&self.0).unwrap_or_else(|error| {
            panic!(
                "failed to remove temporary matrix directory {}: {error}",
                self.0.display()
            )
        });
        // `Drop` is intentionally bypassed after successful explicit cleanup.  This keeps a
        // cleanup failure visible to the test instead of silently retaining temporary files.
        std::mem::forget(self);
    }
}

impl Drop for TemporaryTarget {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run_fixture(manifest: &Path, target_dir: &Path, feature: &str) -> Output {
    let mut command = Command::new("cargo");
    command
        .arg("check")
        .arg("--manifest-path")
        .arg(manifest)
        .arg("--target-dir")
        .arg(target_dir)
        .arg("--no-default-features")
        .arg("--offline")
        .arg("--message-format=short")
        .env("CARGO_TERM_COLOR", "never");
    if !feature.is_empty() {
        command.arg("--features").arg(feature);
    }
    let output = command
        .output()
        .unwrap_or_else(|_| panic!("failed to run cargo check for fixture feature {}", feature));
    output
}

fn copy_fixture_to_temporary_directory(manifest: &Path) -> (TemporaryTarget, PathBuf) {
    let source = manifest
        .parent()
        .unwrap_or_else(|| panic!("fixture manifest has no parent: {}", manifest.display()));
    let destination = TemporaryTarget::new("axutils-feature-fixture");
    copy_fixture_directory(source, destination.path());

    let copied_manifest = destination.path().join("Cargo.toml");
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .to_string_lossy()
        .replace('\\', "/");
    let depth = source
        .strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .unwrap()
        .components()
        .count();
    let relative_root = vec![".."; depth].join("/");
    let manifest_text = fs::read_to_string(&copied_manifest)
        .unwrap_or_else(|_| {
            panic!(
                "failed to read copied fixture: {}",
                copied_manifest.display()
            )
        })
        .replace(
            &format!("path = \"{relative_root}\""),
            &format!("path = \"{repository}\""),
        );
    fs::write(&copied_manifest, manifest_text).unwrap_or_else(|_| {
        panic!(
            "failed to write copied fixture: {}",
            copied_manifest.display()
        )
    });
    (destination, copied_manifest)
}

fn copy_fixture_directory(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap_or_else(|_| {
        panic!(
            "failed to create fixture directory: {}",
            destination.display()
        )
    });
    for entry in fs::read_dir(source)
        .unwrap_or_else(|_| panic!("failed to read fixture directory: {}", source.display()))
    {
        let entry = entry.expect("failed to read fixture entry");
        let name = entry.file_name();
        if name == "Cargo.lock" || name == "target" {
            continue;
        }
        let from = entry.path();
        let to = destination.join(name);
        if entry.file_type().expect("fixture entry type").is_dir() {
            copy_fixture_directory(&from, &to);
        } else {
            fs::copy(&from, &to)
                .unwrap_or_else(|_| panic!("failed to copy fixture {}", from.display()));
        }
    }
}

#[derive(Debug)]
struct FixtureError<'a> {
    code: &'a str,
    message: &'a str,
    source_line: &'a str,
}

impl<'a> FixtureError<'a> {
    fn from_short_diagnostic(line: &'a str, source: &'a str) -> Option<Self> {
        let (location, diagnostic) = line.split_once(": error[")?;
        let (code, message) = diagnostic.split_once("]: ")?;
        let (file_and_line, _) = location.rsplit_once(':')?;
        let (file, line_number) = file_and_line.rsplit_once(':')?;
        if Path::new(file) != Path::new("src/main.rs") {
            return None;
        }
        let index = line_number.parse::<usize>().ok()?.checked_sub(1)?;
        Some(Self {
            code,
            message,
            source_line: source.lines().nth(index)?,
        })
    }

    fn matches(&self, expected: &ExpectedDiagnostic) -> bool {
        let mut quoted = self.message.split('`').skip(1).step_by(2);
        match *expected {
            ExpectedDiagnostic::Member(owner, name) => {
                self.code == "E0599" && quoted.next() == Some(name) && quoted.next() == Some(owner)
            }
            ExpectedDiagnostic::Import(path) => {
                // 短格式在主消息后附加 span 标签；只有主消息列出的导入确实未解析。
                let imports = self.message.split(": ").next().unwrap();
                self.code == "E0432"
                    && imports
                        .split('`')
                        .skip(1)
                        .step_by(2)
                        .any(|item| item == path)
            }
            ExpectedDiagnostic::AtPath(code, subject, path) => {
                // 模块可能先于成员报错，sealed bound 也可能与其他 trait 报在同一行。
                self.code == code
                    && quoted.next() == Some(subject)
                    && self
                        .source_line
                        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':'))
                        .any(|item| item.trim_end_matches("::") == path)
            }
        }
    }
}

fn assert_expected_diagnostics(
    stderr: &str,
    source: &str,
    feature: &str,
    expected: &[ExpectedDiagnostic],
) {
    let diagnostics: Vec<_> = stderr
        .lines()
        .filter_map(|line| FixtureError::from_short_diagnostic(line, source))
        .collect();
    assert!(
        !diagnostics.is_empty(),
        "fixture {feature} did not emit a Rust API diagnostic:\n{stderr}"
    );
    for item in expected {
        assert!(
            diagnostics.iter().any(|error| error.matches(item)),
            "fixture {feature} did not report {item:?}\ndiagnostics: {diagnostics:#?}"
        );
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct TreeKey {
    features: String,
    edges: String,
    invert: Option<String>,
}

type TreeSlot = Arc<OnceLock<String>>;
struct TreeCache {
    slots: BTreeMap<TreeKey, TreeSlot>,
    cargo_calls: usize,
}

static TREE_CACHE: Mutex<TreeCache> = Mutex::new(TreeCache {
    slots: BTreeMap::new(),
    cargo_calls: 0,
});
static FIXTURE_CARGO_CALLS: AtomicUsize = AtomicUsize::new(0);
const LEGACY_TREE_CALL_SITES: usize = 98;
const MIN_TREE_CALL_REDUCTION_PERCENT: usize = 30;

pub(super) fn tree(features: &str) -> String {
    tree_with(features, "normal,build", None)
}

pub(super) fn tree_with(features: &str, edges: &str, invert: Option<&str>) -> String {
    let key = TreeKey {
        features: normalize_feature_csv(features),
        edges: edges.to_owned(),
        invert: invert.map(str::to_owned),
    };
    let slot = {
        let mut cache = TREE_CACHE.lock().expect("TreeCache mutex poisoned");
        cache
            .slots
            .entry(key.clone())
            .or_insert_with(|| Arc::new(OnceLock::new()))
            .clone()
    };
    slot.get_or_init(|| cargo_tree(&key)).clone()
}

pub(super) fn assert_tree_cache_budget() {
    let (unique_keys, cargo_calls) = {
        let cache = TREE_CACHE.lock().expect("TreeCache mutex poisoned");
        (cache.slots.len(), cache.cargo_calls)
    };
    let fixture_calls = FIXTURE_CARGO_CALLS.load(Ordering::Relaxed);
    let saved_calls = LEGACY_TREE_CALL_SITES.saturating_sub(cargo_calls);
    let reduction_percent = saved_calls * 100 / LEGACY_TREE_CALL_SITES;
    eprintln!(
        "FeatureMatrix: fixture cargo calls={fixture_calls}; tree legacy call sites={LEGACY_TREE_CALL_SITES}, unique keys={unique_keys}, cargo calls={cargo_calls}, reduction={reduction_percent}%"
    );
    assert!(
        meets_tree_call_reduction(cargo_calls),
        "TreeCache used {cargo_calls} cargo invocations; expected at least {MIN_TREE_CALL_REDUCTION_PERCENT}% fewer than {LEGACY_TREE_CALL_SITES} legacy call sites"
    );
    assert!(cargo_calls <= unique_keys);
}

fn meets_tree_call_reduction(cargo_calls: usize) -> bool {
    LEGACY_TREE_CALL_SITES.saturating_sub(cargo_calls) * 100
        >= LEGACY_TREE_CALL_SITES * MIN_TREE_CALL_REDUCTION_PERCENT
}

fn cargo_tree(key: &TreeKey) -> String {
    let target = TemporaryTarget::new("axutils-dependency-tree");
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .to_string_lossy()
        .replace('\\', "/");
    let forwarded = key
        .features
        .split(',')
        .filter(|feature| !feature.is_empty())
        .map(|feature| format!("\"axutils/{feature}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let manifest = target.path().join("Cargo.toml");
    fs::write(
        &manifest,
        format!(
            "[package]\nname = \"axutils-dependency-tree\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[features]\nselected = [{forwarded}]\n\n[dependencies]\naxutils = {{ path = \"{repository}\", default-features = false }}\n"
        ),
    )
    .expect("failed to write dependency tree manifest");
    fs::create_dir_all(target.path().join("src")).expect("failed to create dependency tree source");
    fs::write(target.path().join("src/lib.rs"), "")
        .expect("failed to write dependency tree source");

    let mut command = Command::new("cargo");
    command
        .arg("tree")
        .arg("--manifest-path")
        .arg(manifest)
        .arg("--no-default-features")
        .arg("--features")
        .arg("selected")
        .arg("--offline")
        .arg("--edges")
        .arg(&key.edges)
        .env("CARGO_TERM_COLOR", "never");
    if let Some(package) = &key.invert {
        command.arg("--invert").arg(package);
    }
    TREE_CACHE
        .lock()
        .expect("TreeCache mutex poisoned")
        .cargo_calls += 1;
    let output = command.output().expect("failed to run cargo tree");
    assert!(
        output.status.success(),
        "cargo tree failed for {key:?}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let tree = String::from_utf8(output.stdout).expect("cargo tree output must be UTF-8");
    target.cleanup();
    tree
}

pub(super) fn normalize_feature_csv(features: &str) -> String {
    let mut features = features
        .split(',')
        .filter(|feature| !feature.trim().is_empty())
        .map(str::trim)
        .collect::<Vec<_>>();
    features.sort_unstable();
    features.dedup();
    features.join(",")
}

pub(super) fn has_package(tree: &str, package: &str) -> bool {
    tree.lines()
        .flat_map(str::split_whitespace)
        .any(|part| part == package)
}

#[cfg(test)]
mod tests {
    use super::meets_tree_call_reduction;

    #[test]
    #[should_panic(expected = "did not report Member(\"TimeUtils\", \"format_date\")")]
    fn remaining_format_datetime_error_does_not_cover_restored_format_date() {
        use super::ExpectedDiagnostic::Member;
        let source = "let _ = TimeUtils::format_date;\nlet _ = TimeUtils::format_datetime;";
        let stderr = "src/main.rs:2:20: error[E0599]: no associated function named `format_datetime` found for struct `TimeUtils`: help: use `format_date`\n\
                      src/main.rs:1:20: error[E0308]: mismatched types for `format_date` on `TimeUtils`";
        super::assert_expected_diagnostics(
            stderr,
            source,
            "time",
            &[
                Member("TimeUtils", "format_datetime"),
                Member("TimeUtils", "format_date"),
            ],
        );
    }

    #[test]
    fn tree_call_budget_matches_the_thirty_percent_contract() {
        assert!(meets_tree_call_reduction(68));
        assert!(!meets_tree_call_reduction(69));
    }
}
