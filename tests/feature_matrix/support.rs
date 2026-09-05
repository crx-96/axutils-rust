use std::{
    collections::BTreeMap,
    env, fs,
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
    pub(super) diagnostic_tokens: &'static [&'static str],
}

pub(super) fn run_fixture_cases(name: &str, cases: &[FixtureCase]) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("semantic_feature_matrix")
        .join("Cargo.toml");
    let target = TemporaryTarget::new(&format!("axutils-{}-feature-matrix", name));
    let (fixture, copied_manifest) = copy_fixture_to_temporary_directory(&manifest);
    for case in cases {
        FIXTURE_CARGO_CALLS.fetch_add(1, Ordering::Relaxed);
        let output = run_fixture(&copied_manifest, target.path(), case.feature);
        assert_eq!(
            output.status.success(),
            case.expected_success,
            "fixture feature {} unexpected status\nstdout: {}\nstderr: {}",
            case.feature,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        if !case.expected_success {
            assert_expected_diagnostics(&output, case.feature, case.diagnostic_tokens);
        }
    }
    fixture.cleanup();
    target.cleanup();
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
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("axutils")
                && String::from_utf8_lossy(&output.stderr).contains("does not have"),
            "removed provider feature {} failed for an unexpected reason: {}",
            feature,
            String::from_utf8_lossy(&output.stderr)
        );
        probe.cleanup();
    }
    target.cleanup();
}

struct TemporaryTarget(PathBuf);

impl TemporaryTarget {
    fn new(prefix: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        for _ in 0..100 {
            let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path =
                env::temp_dir().join(format!("{}-{}-{}", prefix, std::process::id(), counter));
            if fs::create_dir(&path).is_ok() {
                return Self(path);
            }
        }
        panic!("failed to allocate temporary target for {}", prefix);
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
    // The group owns one private copy. This keeps Windows artifact identity isolated while
    // letting all feature cases reuse the group's Cargo target directory.
    let mut command = Command::new("cargo");
    command
        .arg("check")
        .arg("--manifest-path")
        .arg(manifest)
        .arg("--target-dir")
        .arg(target_dir)
        .arg("--no-default-features")
        .arg("--offline")
        .arg("--message-format=json")
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
    let manifest_text = fs::read_to_string(&copied_manifest)
        .unwrap_or_else(|_| {
            panic!(
                "failed to read copied fixture: {}",
                copied_manifest.display()
            )
        })
        .replace("path = \"../../..\"", &format!("path = \"{}\"", repository));
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

fn assert_expected_diagnostics(output: &Output, feature: &str, tokens: &[&str]) {
    let diagnostics = rust_error_diagnostics(output);
    assert!(
        !diagnostics.is_empty(),
        "fixture {} did not emit a Rust API diagnostic",
        feature
    );
    for token in tokens {
        let token = normalize(token);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| normalize(diagnostic).contains(&token)),
            "fixture {} did not report {}\ndiagnostics: {diagnostics:#?}",
            feature,
            token,
        );
    }
}

fn rust_error_diagnostics(output: &Output) -> Vec<&str> {
    const API_CODES: [&str; 7] = [
        "E0277", "E0412", "E0425", "E0432", "E0433", "E0599", "E0603",
    ];
    std::str::from_utf8(&output.stdout)
        .expect("cargo JSON output must be UTF-8")
        .lines()
        .filter(|line| {
            line.contains(r#""reason":"compiler-message""#)
                && line.contains(r#""level":"error""#)
                && API_CODES
                    .iter()
                    .any(|code| line.contains(&format!(r#""code":"{code}""#)))
        })
        .collect()
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct TreeKey {
    features: String,
    edges: String,
    invert: Option<String>,
}

type TreeSlot = Arc<OnceLock<String>>;
static TREE_CACHE: OnceLock<Mutex<BTreeMap<TreeKey, TreeSlot>>> = OnceLock::new();
static TREE_CARGO_CALLS: AtomicUsize = AtomicUsize::new(0);
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
        let cache = TREE_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
        let mut cache = cache.lock().expect("TreeCache mutex poisoned");
        cache
            .entry(key.clone())
            .or_insert_with(|| Arc::new(OnceLock::new()))
            .clone()
    };
    slot.get_or_init(|| cargo_tree(&key)).clone()
}

pub(super) fn assert_tree_cache_budget() {
    let cache = TREE_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let unique_keys = cache.lock().expect("TreeCache mutex poisoned").len();
    let cargo_calls = TREE_CARGO_CALLS.load(Ordering::Relaxed);
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
    TREE_CARGO_CALLS.fetch_add(1, Ordering::Relaxed);
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

fn normalize_feature_csv(features: &str) -> String {
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
    fn tree_call_budget_matches_the_thirty_percent_contract() {
        assert!(meets_tree_call_reduction(68));
        assert!(!meets_tree_call_reduction(69));
    }
}
