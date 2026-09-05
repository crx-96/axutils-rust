use std::collections::BTreeMap;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use super::parse::{canonical_features, compile_fail_diagnostic_tokens, metadata_for};
use super::{
    BlockMetadata, CompileMode, DirectDependency, Document, Fence,
    LEGACY_PER_BLOCK_CARGO_PROCESSES, ROOT_MANIFEST,
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DirectDependencyKey {
    name: &'static str,
    package: Option<&'static str>,
    version: &'static str,
    default_features: bool,
    features: Vec<&'static str>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct GroupKey {
    axutils_features: Vec<&'static str>,
    direct_dependencies: Vec<DirectDependencyKey>,
}

#[derive(Debug)]
struct CompileCase {
    key: String,
    source: String,
    cfg_features: Vec<String>,
    start_line: usize,
}

#[derive(Default)]
struct Counts {
    compiled: usize,
    no_run: usize,
    excluded: usize,
    compile_fail: usize,
    cargo_processes: usize,
}

pub(super) fn compile_documents(documents: &[Document]) {
    let mut workspace = TempDir::new("axutils-docs-examples").unwrap_or_else(|error| {
        panic!("failed to create isolated docs-example workspace: {error}")
    });
    let target_dir = workspace.path().join("target");
    fs::create_dir_all(&target_dir)
        .unwrap_or_else(|error| panic!("failed to create isolated CARGO_TARGET_DIR: {error}"));

    let filter = env::var("AXUTILS_DOCS_EXAMPLE_FILTER").ok();
    let start = env::var("AXUTILS_DOCS_EXAMPLE_START").ok();
    let mut started = start.is_none();
    let mut counts = Counts::default();
    let mut groups = BTreeMap::<GroupKey, Vec<CompileCase>>::new();
    let mut failures = Vec::new();

    for document in documents {
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

            let primary = metadata_for(&document.path, block_number, fence)
                .expect("metadata was validated above");
            collect_case(
                document,
                block_number,
                fence,
                &primary,
                &mut counts,
                &mut groups,
                &mut failures,
            );
        }
    }
    assert!(
        started,
        "AXUTILS_DOCS_EXAMPLE_START did not match any block: {:?}",
        start
    );

    let group_count = groups.len();
    for (group_number, (group, cases)) in groups.into_iter().enumerate() {
        if let Err(error) = run_group(
            workspace.path(),
            &target_dir,
            group_number + 1,
            &group,
            &cases,
            &mut counts,
        ) {
            failures.push(error);
        }
    }
    if let Err(error) = workspace.cleanup() {
        failures.push(format!(
            "failed to remove temporary docs workspace: {error}"
        ));
    }
    if let Some(case) = failures.into_iter().next() {
        panic!("{case}");
    }

    let baseline_processes = counts.compiled + counts.no_run;
    let saved_processes = baseline_processes.saturating_sub(counts.cargo_processes);
    let legacy_saved_processes =
        LEGACY_PER_BLOCK_CARGO_PROCESSES.saturating_sub(counts.cargo_processes);
    let current_reduction_percent = (saved_processes * 100)
        .checked_div(baseline_processes)
        .unwrap_or(0);
    let legacy_reduction_percent = legacy_saved_processes * 100 / LEGACY_PER_BLOCK_CARGO_PROCESSES;
    println!(
        "docs_examples summary: compiled={}, no_run={}, explicitly_excluded={}, compile_fail={}, groups={}, cargo_processes={}, current_per_block_baseline={}, current_saved_processes={}, current_reduction_percent={}, legacy_per_block_baseline={}, legacy_saved_processes={}, legacy_reduction_percent={}",
        counts.compiled,
        counts.no_run,
        counts.excluded,
        counts.compile_fail,
        group_count,
        counts.cargo_processes,
        baseline_processes,
        saved_processes,
        current_reduction_percent,
        LEGACY_PER_BLOCK_CARGO_PROCESSES,
        legacy_saved_processes,
        legacy_reduction_percent,
    );
    if filter.is_none() {
        assert!(
            counts.cargo_processes * 2 <= LEGACY_PER_BLOCK_CARGO_PROCESSES,
            "full docs harness did not reduce Cargo processes by at least 50% from the recorded per-block baseline"
        );
    }
}

fn collect_case(
    document: &Document,
    block_number: usize,
    fence: &Fence,
    metadata: &BlockMetadata,
    counts: &mut Counts,
    groups: &mut BTreeMap<GroupKey, Vec<CompileCase>>,
    failures: &mut Vec<String>,
) {
    report_block(document, block_number, metadata);
    let canonical_features = canonical_features(metadata);
    let case = CompileCase {
        key: metadata.key.clone(),
        source: wrap_rust_source(&fence.body),
        cfg_features: referenced_cfg_features(&fence.body),
        start_line: fence.start_line,
    };
    if let Err(error) = validate_cfg_features(&case, &canonical_features) {
        failures.push(error);
        return;
    }
    match metadata.mode {
        CompileMode::Compiled => counts.compiled += 1,
        CompileMode::NoRun => counts.no_run += 1,
        CompileMode::ExplicitlyExcluded => {
            counts.excluded += 1;
            return;
        }
        CompileMode::CompileFail => {
            counts.compile_fail += 1;
            if let Err(error) = run_compile_fail_case(case, metadata, counts) {
                failures.push(error);
            }
            return;
        }
    }
    groups.entry(group_key(metadata)).or_default().push(case);
}

fn group_key(metadata: &BlockMetadata) -> GroupKey {
    let mut axutils_features = canonical_features(metadata);
    axutils_features.sort_unstable();
    axutils_features.dedup();
    let mut direct_dependencies = metadata
        .direct_dependencies
        .iter()
        .map(DirectDependencyKey::from)
        .collect::<Vec<_>>();
    direct_dependencies.sort_unstable();
    direct_dependencies.dedup();
    GroupKey {
        axutils_features,
        direct_dependencies,
    }
}

impl From<&DirectDependency> for DirectDependencyKey {
    fn from(value: &DirectDependency) -> Self {
        let mut features = value.features.to_vec();
        features.sort_unstable();
        features.dedup();
        Self {
            name: value.name,
            package: value.package,
            version: value.version,
            default_features: value.default_features,
            features,
        }
    }
}

fn run_group(
    workspace: &Path,
    target_dir: &Path,
    group_number: usize,
    group: &GroupKey,
    cases: &[CompileCase],
    counts: &mut Counts,
) -> Result<(), String> {
    let group_dir = workspace.join(format!("group-{group_number:04}"));
    let source_dir = group_dir.join("src").join("bin");
    fs::create_dir_all(&source_dir)
        .map_err(|error| format!("failed to create group source directory: {error}"))?;
    let manifest = group_dir.join("Cargo.toml");
    write_group_manifest(&manifest, group)
        .map_err(|error| format!("failed to write group manifest: {error}"))?;
    for (index, case) in cases.iter().enumerate() {
        fs::write(
            source_dir.join(format!("{}.rs", bin_name(index))),
            &case.source,
        )
        .map_err(|error| format!("{}: failed to write group source: {error}", case.key))?;
    }

    counts.cargo_processes += 1;
    let output = cargo_check(&manifest, target_dir, &group.axutils_features, None)?;
    if output.status.success() {
        return Ok(());
    }

    let mut errors = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        counts.cargo_processes += 1;
        let output = cargo_check(
            &manifest,
            target_dir,
            &group.axutils_features,
            Some(&bin_name(index)),
        )?;
        if !output.status.success() {
            errors.push(format!(
                "{} failed:\n{}",
                case.key,
                redact_diagnostic(&combined_output(&output), &[&group_dir, target_dir])
            ));
        }
    }
    if errors.is_empty() {
        errors.push(format!(
            "group {group_number} failed but no individual bin reproduced it:\n{}",
            redact_diagnostic(&combined_output(&output), &[&group_dir, target_dir])
        ));
    }
    Err(errors.join("\n\n"))
}

fn run_compile_fail_case(
    case: CompileCase,
    metadata: &BlockMetadata,
    counts: &mut Counts,
) -> Result<(), String> {
    let mut workspace = TempDir::new("axutils-docs-compile-fail")
        .map_err(|error| format!("{}: failed to create temporary crate: {error}", case.key))?;
    let target_dir = workspace.path().join("target");
    let group = group_key(metadata);
    let manifest = workspace.path().join("Cargo.toml");
    write_group_manifest(&manifest, &group)
        .map_err(|error| format!("{}: failed to write manifest: {error}", case.key))?;
    let source = workspace
        .path()
        .join("src")
        .join("bin")
        .join("case_0001.rs");
    fs::create_dir_all(source.parent().expect("bin source parent exists"))
        .map_err(|error| format!("{}: failed to create source directory: {error}", case.key))?;
    fs::write(&source, case.source)
        .map_err(|error| format!("{}: failed to write source: {error}", case.key))?;
    counts.cargo_processes += 1;
    let tokens = compile_fail_diagnostic_tokens(&metadata.key).ok_or_else(|| {
        format!(
            "{} compile_fail metadata has no expected diagnostic token",
            metadata.key
        )
    })?;
    let output = cargo_check(
        &manifest,
        &target_dir,
        &group.axutils_features,
        Some("case_0001"),
    )?;
    let result = if output.status.success() {
        Err(format!(
            "{} compile_fail case unexpectedly compiled",
            metadata.key
        ))
    } else {
        assert_expected_diagnostics(&combined_output(&output), tokens, &metadata.key)
    };
    let cleanup = workspace.cleanup();
    match (result, cleanup) {
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(format!("{}; cleanup failed: {error}", metadata.key)),
        (Err(error), Err(cleanup_error)) => {
            Err(format!("{error}; cleanup failed: {cleanup_error}"))
        }
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn write_group_manifest(manifest: &Path, group: &GroupKey) -> io::Result<()> {
    let root_path = Path::new(ROOT_MANIFEST)
        .to_string_lossy()
        .replace('\\', "/");
    let mut text = String::from(
        "[package]\nname = \"axutils_docs_example\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[features]\ndefault = []\n",
    );
    for feature in &group.axutils_features {
        writeln!(text, "{feature} = [\"axutils/{feature}\"]")
            .expect("writing to String cannot fail");
    }
    text.push_str("\n[dependencies]\n");
    writeln!(
        text,
        "axutils = {{ path = \"{root_path}\", default-features = false }}"
    )
    .expect("writing to String cannot fail");
    for dependency in &group.direct_dependencies {
        writeln!(text, "{}", dependency_manifest_line(dependency))
            .expect("writing to String cannot fail");
    }
    fs::write(manifest, text)
}

fn dependency_manifest_line(dependency: &DirectDependencyKey) -> String {
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
        write!(line, ", features = [{}]", quoted_list(&dependency.features))
            .expect("writing to String cannot fail");
    }
    line.push_str(" }");
    line
}

fn cargo_check(
    manifest: &Path,
    target_dir: &Path,
    features: &[&str],
    bin: Option<&str>,
) -> Result<std::process::Output, String> {
    let mut command = Command::new("cargo");
    command
        .arg("check")
        .arg("--offline")
        .arg("--manifest-path")
        .arg(manifest)
        .env("CARGO_TARGET_DIR", target_dir)
        .env("CARGO_TERM_COLOR", "never");
    if !features.is_empty() {
        command.arg("--features").arg(features.join(","));
    }
    if let Some(bin) = bin {
        command.arg("--bin").arg(bin);
    } else {
        command.arg("--bins");
    }
    command
        .output()
        .map_err(|error| format!("failed to start cargo: {error}"))
}

fn report_block(document: &Document, block_number: usize, metadata: &BlockMetadata) {
    let features = canonical_features(metadata).join(",");
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
    println!(
        "{} #{} axutils=[{}] direct=[{}] status={}",
        document.path,
        block_number,
        if features.is_empty() { "-" } else { &features },
        dependencies,
        metadata.mode.as_str(),
    );
}

fn bin_name(index: usize) -> String {
    format!("case_{:04}", index + 1)
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
    if code.contains('?') {
        source.push_str("fn main() -> Result<(), Box<dyn std::error::Error>> {\n");
    } else {
        source.push_str("fn main() {\n");
    }
    source.push_str(&code);
    source.push('\n');
    if code.contains('?') {
        if !code.trim_end().ends_with(';') && !code.trim_end().ends_with('}') {
            source.push_str(";\n");
        }
        source.push_str("Ok(())\n");
    }
    source.push_str("}\n");
    source
}

fn referenced_cfg_features(source: &str) -> Vec<String> {
    let mut features = BTreeMap::<String, ()>::new();
    let mut remaining = source;
    while let Some(index) = remaining.find("feature") {
        remaining = &remaining[index + "feature".len()..];
        let candidate = remaining.trim_start();
        let Some(candidate) = candidate.strip_prefix('=') else {
            continue;
        };
        let candidate = candidate.trim_start();
        let Some(quote) = candidate
            .chars()
            .next()
            .filter(|quote| *quote == '\"' || *quote == '\'')
        else {
            continue;
        };
        let value = &candidate[quote.len_utf8()..];
        let Some(end) = value.find(quote) else {
            continue;
        };
        let feature = &value[..end];
        if !feature.is_empty()
            && feature.chars().all(|character| {
                character.is_ascii_alphanumeric() || character == '_' || character == '-'
            })
        {
            features.insert(feature.to_owned(), ());
        }
        remaining = &value[end + quote.len_utf8()..];
    }
    features.into_keys().collect()
}

fn validate_cfg_features(case: &CompileCase, active_features: &[&str]) -> Result<(), String> {
    let inactive = case
        .cfg_features
        .iter()
        .filter(|feature| !active_features.contains(&feature.as_str()))
        .collect::<Vec<_>>();
    if inactive.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{} (fence starts at line {}) references cfg features {:?} that are not activated by this case {:?}",
        case.key, case.start_line, inactive, active_features
    ))
}

fn contains_main_function(code: &str) -> bool {
    code.lines().any(|line| {
        let trimmed = line.trim_start();
        (trimmed.starts_with("fn main") || trimmed.starts_with("async fn main"))
            && trimmed.contains('(')
    })
}

fn combined_output(output: &std::process::Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_expected_diagnostics(
    diagnostic: &str,
    expected: &[&str],
    key: &str,
) -> Result<(), String> {
    let normalized = normalize_diagnostic(diagnostic);
    let missing = expected
        .iter()
        .filter(|token| !normalized.contains(&normalize_diagnostic(token)))
        .copied()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{key} compile_fail diagnostic is missing tokens {missing:?}:\n{}",
            redact_diagnostic(diagnostic, &[])
        ))
    }
}

fn normalize_diagnostic(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn redact_diagnostic(diagnostic: &str, paths: &[&Path]) -> String {
    let mut redacted = diagnostic.to_owned();
    for path in paths {
        let path = path.to_string_lossy();
        redacted = redacted.replace(path.as_ref(), "<temporary-path>");
    }
    diagnostic_lines(&redacted).join("\n")
}

fn diagnostic_lines(diagnostic: &str) -> Vec<String> {
    diagnostic
        .lines()
        .take(120)
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            if lower.contains("password")
                || lower.contains("secret")
                || lower.contains("authorization")
                || lower.contains("cookie")
                || lower.contains("token")
                || lower.contains("api_key")
                || lower.contains("api-key")
                || lower.contains("credential")
                || lower.contains("bearer")
            {
                "<redacted sensitive diagnostic line>".to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect()
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

    fn cleanup(&mut self) -> io::Result<()> {
        if self.path.as_os_str().is_empty() {
            return Ok(());
        }
        let original = self.path.clone();
        let mut last_error = None;
        for attempt in 0..5 {
            match fs::remove_dir_all(&self.path) {
                Ok(()) => {
                    self.path.clear();
                    return Ok(());
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    self.path.clear();
                    return Ok(());
                }
                Err(error) => last_error = Some(error),
            }
            std::thread::sleep(std::time::Duration::from_millis(25 * (attempt + 1)));
        }
        Err(io::Error::other(format!(
            "{} remained after cleanup retries: {}",
            original.display(),
            last_error.expect("cleanup retry records an error")
        )))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{redact_diagnostic, referenced_cfg_features, validate_cfg_features, CompileCase};

    #[test]
    fn inactive_cfg_feature_is_rejected() {
        let source = r#"#[cfg(feature = "http-async")]
fn only_with_async_http() {}"#;
        let case = CompileCase {
            key: "docs/examples/http.md#test".to_owned(),
            source: source.to_owned(),
            cfg_features: referenced_cfg_features(source),
            start_line: 7,
        };

        let error = validate_cfg_features(&case, &["http"])
            .expect_err("inactive cfg feature must be rejected");
        assert!(error.contains("http-async"));
        assert!(error.contains("line 7"));
        validate_cfg_features(&case, &["http", "http-async"])
            .expect("active cfg feature must pass");
    }

    #[test]
    fn diagnostics_are_path_redacted_and_sensitive_lines_removed() {
        let temporary = Path::new("C:\\temporary\\docs-example");
        let diagnostic =
            "failed at C:\\temporary\\docs-example\\src\\main.rs\npassword=do-not-print";
        let redacted = redact_diagnostic(diagnostic, &[temporary]);

        assert!(redacted.contains("<temporary-path>\\src\\main.rs"));
        assert!(redacted.contains("<redacted sensitive diagnostic line>"));
        assert!(!redacted.contains("do-not-print"));
    }
}
