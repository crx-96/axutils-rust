use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use super::{
    BlockMetadata, CompileMode, CompileSettings, Document, Fence, DOCUMENT_METADATA, MODULE_MAP,
    ROOT_MANIFEST, VALID_AXUTILS_FEATURES,
};

const NON_RUST_EXCLUSION: &str =
    "非 Rust fence（配置、签名、文本或命令），不作为 Rust crate 编译。";
const RUST_IGNORE_EXCLUSION: &str =
    "rust,ignore fence 仅用于展示内部或伪代码路径，不进入离线编译。";

pub(super) fn load_documents() -> io::Result<Vec<Document>> {
    validate_examples_directory()?;
    let mut documents = Vec::with_capacity(DOCUMENT_METADATA.len());
    for metadata in DOCUMENT_METADATA {
        let path = metadata.path;
        let absolute = Path::new(ROOT_MANIFEST).join(path);
        let source = fs::read_to_string(&absolute)?;
        let fences = parse_fences(&source).map_err(|error| {
            io::Error::new(io::ErrorKind::InvalidData, format!("{path}: {error}"))
        })?;
        documents.push(Document {
            path: path.to_owned(),
            fences,
        });
    }
    Ok(documents)
}

fn validate_examples_directory() -> io::Result<()> {
    let directory = Path::new(ROOT_MANIFEST).join("docs").join("examples");
    let mut discovered = BTreeSet::new();
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() || entry.path().extension().is_none_or(|ext| ext != "md") {
            continue;
        }
        let file_name = entry.file_name().into_string().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "docs/examples contains a non-UTF-8 filename",
            )
        })?;
        discovered.insert(format!("docs/examples/{file_name}"));
    }
    let configured = DOCUMENT_METADATA
        .iter()
        .filter(|metadata| metadata.path.starts_with("docs/examples/"))
        .map(|metadata| metadata.path.to_owned())
        .collect::<BTreeSet<_>>();
    if discovered == configured {
        return Ok(());
    }
    let missing = discovered.difference(&configured).collect::<Vec<_>>();
    let stale = configured.difference(&discovered).collect::<Vec<_>>();
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "docs/examples and document metadata differ; unconfigured={missing:?}, missing_files={stale:?}"
        ),
    ))
}

pub(super) fn parse_fences(source: &str) -> Result<Vec<Fence>, String> {
    let lines: Vec<&str> = source.lines().collect();
    let mut fences = Vec::new();
    let mut open: Option<(u8, usize, usize, String, Vec<String>)> = None;

    for (index, line) in lines.iter().enumerate() {
        let line_number = index + 1;
        let line_without_cr = line.strip_suffix('\r').unwrap_or(line);
        let trimmed = line_without_cr.trim_start();
        let Some(marker) = trimmed.as_bytes().first().copied() else {
            if let Some((_, _, _, _, body)) = open.as_mut() {
                body.push((*line).to_owned());
            }
            continue;
        };
        if marker != b'`' && marker != b'~' {
            if let Some((_, _, _, _, body)) = open.as_mut() {
                body.push((*line).to_owned());
            }
            continue;
        }
        let marker_len = trimmed.bytes().take_while(|byte| *byte == marker).count();
        if marker_len < 3 {
            if let Some((_, _, _, _, body)) = open.as_mut() {
                body.push((*line).to_owned());
            }
            continue;
        }

        match open.as_mut() {
            Some((open_marker, open_len, _, _, _))
                if *open_marker == marker
                    && marker_len >= *open_len
                    && trimmed[marker_len..].trim().is_empty() =>
            {
                let (_, _, start_line, info, body) = open.take().expect("open fence exists");
                fences.push(Fence {
                    info,
                    body: body.join("\n"),
                    start_line,
                });
            }
            Some((_, _, _, _, body)) => body.push((*line).to_owned()),
            None => {
                let info = trimmed[marker_len..].trim().to_owned();
                open = Some((marker, marker_len, line_number, info, Vec::new()));
            }
        }
    }

    if let Some((_, _, start_line, _, _)) = open {
        return Err(format!("unclosed fence opened at line {start_line}"));
    }
    Ok(fences)
}

pub(super) fn validate_metadata(documents: &[Document]) {
    validate_document_sets(documents);

    for document_metadata in DOCUMENT_METADATA {
        assert_known_features(
            document_metadata.defaults,
            &format!("{} defaults", document_metadata.path),
        );
        assert_direct_dependencies(
            document_metadata.defaults,
            &format!("{} defaults", document_metadata.path),
        );

        let document = documents
            .iter()
            .find(|document| document.path == document_metadata.path)
            .expect("document sets were checked above");
        let mut override_numbers = BTreeSet::new();
        for block_override in document_metadata.overrides {
            assert!(
                override_numbers.insert(block_override.fence_number),
                "{} has duplicate override for fence #{}",
                document_metadata.path,
                block_override.fence_number
            );
            let fence = document
                .fences
                .get(block_override.fence_number.saturating_sub(1))
                .unwrap_or_else(|| {
                    panic!(
                        "{} override references missing fence #{} (document has {})",
                        document_metadata.path,
                        block_override.fence_number,
                        document.fences.len()
                    )
                });
            assert_eq!(
                fence_language(&fence.info),
                Some("rust"),
                "{}#{} overrides a non-Rust fence `{}`",
                document_metadata.path,
                block_override.fence_number,
                fence.info
            );
            let label = format!(
                "{}#{} override",
                document_metadata.path, block_override.fence_number
            );
            assert_known_features(block_override.settings, &label);
            assert_direct_dependencies(block_override.settings, &label);
        }

        for (index, fence) in document.fences.iter().enumerate() {
            let block_number = index + 1;
            let metadata = metadata_for(&document.path, block_number, fence)
                .expect("document metadata was checked above");
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
            if metadata.mode == CompileMode::CompileFail {
                assert!(
                    compile_fail_diagnostic_tokens(&metadata.key).is_some(),
                    "{}: compile_fail fence requires stable diagnostic tokens",
                    metadata.key
                );
            }
        }
    }
}

fn validate_document_sets(documents: &[Document]) {
    let configured = DOCUMENT_METADATA
        .iter()
        .map(|metadata| metadata.path)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        configured.len(),
        DOCUMENT_METADATA.len(),
        "duplicate document metadata path"
    );

    let loaded = documents
        .iter()
        .map(|document| document.path.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        loaded, configured,
        "loaded documents and document metadata differ"
    );

    let configured_examples = DOCUMENT_METADATA
        .iter()
        .filter(|metadata| metadata.path.starts_with("docs/examples/"))
        .map(|metadata| metadata.path)
        .collect::<BTreeSet<_>>();
    let mapped = module_map_document_paths();
    assert_eq!(
        mapped, configured_examples,
        "module map example links and document metadata differ"
    );
}

fn module_map_document_paths() -> BTreeSet<&'static str> {
    let mut paths = BTreeSet::new();
    let mut remainder = MODULE_MAP;
    while let Some(index) = remainder.find("examples/") {
        let suffix = &remainder[index..];
        let end = suffix
            .char_indices()
            .find(|(_, character)| {
                !character.is_ascii_alphanumeric()
                    && *character != '_'
                    && *character != '-'
                    && *character != '.'
                    && *character != '/'
            })
            .map_or(suffix.len(), |(offset, _)| offset);
        let relative = &suffix[..end];
        if relative.ends_with(".md") {
            let expected = DOCUMENT_METADATA
                .iter()
                .find(|metadata| metadata.path.strip_prefix("docs/") == Some(relative))
                .map(|metadata| metadata.path)
                .unwrap_or_else(|| {
                    panic!("module map references unconfigured document: {relative}")
                });
            paths.insert(expected);
        }
        remainder = &suffix[end.max("examples/".len())..];
    }
    paths
}

pub(super) fn metadata_for(
    document_path: &str,
    block_number: usize,
    fence: &Fence,
) -> Option<BlockMetadata> {
    let document = DOCUMENT_METADATA
        .iter()
        .find(|metadata| metadata.path == document_path)?;
    let settings = document
        .overrides
        .iter()
        .find(|block_override| block_override.fence_number == block_number)
        .map_or(document.defaults, |block_override| block_override.settings);
    let (mode, exclusion_reason) = compile_mode(&fence.info);
    let (axutils_features, direct_dependencies) = if mode == CompileMode::ExplicitlyExcluded {
        (&[][..], &[][..])
    } else {
        (settings.axutils_features, settings.direct_dependencies)
    };
    Some(BlockMetadata {
        key: format!("{document_path}#{block_number}"),
        axutils_features,
        direct_dependencies,
        mode,
        exclusion_reason,
    })
}

pub(super) fn canonical_features(metadata: &BlockMetadata) -> Vec<&'static str> {
    metadata
        .axutils_features
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn compile_fail_diagnostic_tokens(key: &str) -> Option<&'static [&'static str]> {
    // compile_fail fence 必须在这里登记稳定的 rustc 诊断 token，并由 runner 单独编译。
    const TOKENS: &[(&str, &[&str])] = &[];
    TOKENS
        .iter()
        .find_map(|(known_key, tokens)| (*known_key == key).then_some(*tokens))
}

fn compile_mode(info: &str) -> (CompileMode, Option<&'static str>) {
    if fence_language(info) != Some("rust") {
        return (CompileMode::ExplicitlyExcluded, Some(NON_RUST_EXCLUSION));
    }
    if fence_has_flag(info, "ignore") {
        return (CompileMode::ExplicitlyExcluded, Some(RUST_IGNORE_EXCLUSION));
    }
    if fence_has_flag(info, "compile_fail") {
        return (CompileMode::CompileFail, None);
    }
    if fence_has_flag(info, "no_run") {
        return (CompileMode::NoRun, None);
    }
    (CompileMode::Compiled, None)
}

fn assert_known_features(settings: CompileSettings, label: &str) {
    let mut seen = BTreeSet::new();
    for feature in settings.axutils_features {
        assert!(
            VALID_AXUTILS_FEATURES.contains(feature),
            "{label} lists unknown axutils feature `{feature}`"
        );
        assert!(
            seen.insert(*feature),
            "{label} lists duplicate axutils feature `{feature}`"
        );
    }
}

fn assert_direct_dependencies(settings: CompileSettings, label: &str) {
    let mut seen = BTreeSet::new();
    for dependency in settings.direct_dependencies {
        assert!(
            !dependency.name.trim().is_empty() && !dependency.version.trim().is_empty(),
            "{label} has an incomplete direct dependency"
        );
        assert_ne!(
            dependency.name, "axutils",
            "{label} must not redeclare the harness dependency"
        );
        assert!(
            seen.insert(dependency.name),
            "{label} lists duplicate direct dependency `{}`",
            dependency.name
        );
    }
}

fn fence_language(info: &str) -> Option<&str> {
    info.split(|character: char| character == ',' || character.is_ascii_whitespace())
        .find(|part| !part.is_empty())
}

fn fence_has_flag(info: &str, flag: &str) -> bool {
    info.split(|character: char| character == ',' || character.is_ascii_whitespace())
        .any(|part| part == flag)
}

#[cfg(test)]
mod tests {
    use super::parse_fences;

    #[test]
    fn unclosed_fence_is_rejected() {
        let error =
            parse_fences("before\n```rust\nfn main() {}").expect_err("unclosed fence must fail");
        assert_eq!(error, "unclosed fence opened at line 2");
    }
}
