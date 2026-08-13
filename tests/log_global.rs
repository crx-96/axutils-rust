#![cfg(feature = "logging")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Barrier};

use axutils::{LogConfig, LogError, LogFileConfig, LogLevel, LogRotation, LogUtils};

const EVENT_SENTINEL: &str = "AXUTILS_LOG_EVENT_SENTINEL";
const INFO_SENTINEL: &str = "AXUTILS_LOG_INFO_SENTINEL";
const ERROR_SENTINEL: &str = "AXUTILS_LOG_ERROR_SENTINEL";
const PATH_SENTINEL: &str = "AXUTILS_LOG_PATH_SENTINEL";

#[test]
fn global_tracing_behaviors_are_isolated_in_child_processes() {
    let mut cases = vec![
        "silent",
        "stdout",
        "file",
        "relative",
        "dual",
        "rotation-minutely",
        "rotation-hourly",
        "rotation-daily",
        "invalid",
        "invalid-output",
        "file-error",
        "repeat",
        "concurrency",
        "level",
    ];
    #[cfg(windows)]
    cases.push("non-utf8");
    for case in cases {
        let root = unique_temp_dir(case);
        let output = run_child(case, &root);
        assert!(
            output.status.success(),
            "log child case `{case}` failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        match case {
            "silent" => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                assert!(!stdout.contains(EVENT_SENTINEL));
                assert!(!String::from_utf8_lossy(&output.stderr).contains(EVENT_SENTINEL));
                assert_eq!(fs::read_dir(&root).expect("read silent root").count(), 0);
            }
            "stdout" => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                assert_eq!(count_occurrences(&stdout, EVENT_SENTINEL), 1);
                assert!(!stdout.contains('\u{1b}'));
            }
            "level" => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                assert!(!stdout.contains(INFO_SENTINEL));
                assert!(stdout.contains(ERROR_SENTINEL));
                assert!(!stdout.contains('\u{1b}'));
            }
            "file" | "relative" | "dual" => {
                let contents =
                    fs::read_to_string(root.join("application.log")).expect("read exact log file");
                assert_eq!(count_occurrences(&contents, EVENT_SENTINEL), 1);
                assert!(!contents.contains('\u{1b}'));
                if case == "dual" {
                    assert_eq!(
                        count_occurrences(&String::from_utf8_lossy(&output.stdout), EVENT_SENTINEL),
                        1
                    );
                }
            }
            "rotation-minutely" | "rotation-hourly" | "rotation-daily" => {
                let files = fs::read_dir(&root)
                    .expect("read rotation root")
                    .filter_map(Result::ok)
                    .filter(|entry| entry.path().is_file())
                    .collect::<Vec<_>>();
                assert_eq!(files.len(), 1);
                let file_name = files[0]
                    .file_name()
                    .to_str()
                    .expect("UTF-8 rotation file name")
                    .to_owned();
                assert_rotation_filename(case, &file_name);
                let contents = fs::read_to_string(files[0].path()).expect("read rotated file");
                assert_eq!(count_occurrences(&contents, EVENT_SENTINEL), 1);
                assert!(!contents.contains('\u{1b}'));
            }
            "repeat" => assert!(!root.join(PATH_SENTINEL).exists()),
            _ => {}
        }
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
#[ignore = "由父测试以独立进程方式执行"]
fn child_process_case() {
    let case = std::env::var("AXUTILS_LOG_CASE").expect("child case");
    let root = PathBuf::from(std::env::var("AXUTILS_LOG_ROOT").expect("child root"));
    match case.as_str() {
        "silent" => silent_case(),
        "stdout" => stdout_case(),
        "file" => file_case(&root, false),
        "relative" => relative_file_case(&root),
        "dual" => file_case(&root, true),
        "rotation-minutely" => rotation_case(&root, LogRotation::Minutely),
        "rotation-hourly" => rotation_case(&root, LogRotation::Hourly),
        "rotation-daily" => rotation_case(&root, LogRotation::Daily),
        "invalid" => invalid_case(),
        "invalid-output" => invalid_output_case(),
        "file-error" => file_error_case(&root),
        #[cfg(windows)]
        "non-utf8" => non_utf8_case(),
        "repeat" => repeat_case(&root),
        "concurrency" => concurrency_case(),
        "level" => level_case(),
        other => panic!("unknown log child case: {other}"),
    }
}

fn run_child(case: &str, root: &Path) -> Output {
    let executable = std::env::current_exe().expect("integration test executable");
    Command::new(executable)
        .arg("--ignored")
        .arg("--exact")
        .arg("child_process_case")
        .arg("--nocapture")
        .env("AXUTILS_LOG_CASE", case)
        .env("AXUTILS_LOG_ROOT", root)
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .expect("spawn isolated log test")
}

fn stdout_case() {
    LogUtils::init(LogConfig::default()).expect("stdout init");
    tracing::info!(target: "axutils::test", message = EVENT_SENTINEL);
}

fn silent_case() {
    tracing::info!(target: "axutils::test", message = EVENT_SENTINEL);
    assert!(!LogUtils::is_initialized());
}

fn file_case(root: &Path, dual: bool) {
    fs::create_dir_all(root).expect("create log root");
    let path = root.join("application.log");
    let config = LogConfig::new()
        .with_stdout(dual)
        .with_file(LogFileConfig::new(&path).with_rotation(LogRotation::Never));
    LogUtils::init(config).expect("file init");
    tracing::info!(target: "axutils::test", message = EVENT_SENTINEL);
    assert!(path.exists(), "never rotation should use exact file name");
}

fn relative_file_case(root: &Path) {
    fs::create_dir_all(root).expect("create relative log root");
    std::env::set_current_dir(root).expect("set relative log current directory");
    LogUtils::init(
        LogConfig::new()
            .with_stdout(false)
            .with_file(LogFileConfig::new("application.log").with_rotation(LogRotation::Never)),
    )
    .expect("relative file init");
    tracing::info!(target: "axutils::test", message = EVENT_SENTINEL);
    assert!(Path::new("application.log").exists());
}

fn rotation_case(root: &Path, rotation: LogRotation) {
    fs::create_dir_all(root).expect("create rotation root");
    let path = root.join("rotation.log");
    LogUtils::init(
        LogConfig::new()
            .with_stdout(false)
            .with_file(LogFileConfig::new(&path).with_rotation(rotation)),
    )
    .expect("rotation init");
    tracing::info!(target: "axutils::test", message = EVENT_SENTINEL);
    assert!(
        fs::read_dir(root)
            .expect("read rotation root")
            .filter_map(Result::ok)
            .any(|entry| entry.path().is_file()),
        "rotation should create a file"
    );
}

fn invalid_case() {
    let result = LogUtils::init(
        LogConfig::new()
            .with_stdout(false)
            .with_file(LogFileConfig::new(PathBuf::new())),
    );
    assert!(matches!(&result, Err(LogError::InvalidPath)));
    assert!(!LogUtils::is_initialized());
    assert!(!result
        .as_ref()
        .expect_err("invalid path should fail")
        .to_string()
        .contains(PATH_SENTINEL));
    LogUtils::init(LogConfig::default()).expect("valid init after invalid path");
    assert!(LogUtils::is_initialized());
}

fn invalid_output_case() {
    let result = LogUtils::init(LogConfig::new().with_stdout(false));
    assert!(matches!(&result, Err(LogError::InvalidConfig)));
    assert!(!LogUtils::is_initialized());
    LogUtils::init(LogConfig::default()).expect("valid init after invalid output");
    assert!(LogUtils::is_initialized());
}

fn file_error_case(root: &Path) {
    fs::create_dir_all(root).expect("create file error root");
    let blocked = root.join("blocked");
    fs::write(&blocked, b"not a directory").expect("create blocking file");
    let result = LogUtils::init(
        LogConfig::new()
            .with_stdout(false)
            .with_file(LogFileConfig::new(blocked.join(PATH_SENTINEL))),
    );
    assert!(matches!(&result, Err(LogError::FileInit { .. })));
    assert!(!LogUtils::is_initialized());
    let display = result
        .as_ref()
        .expect_err("file init should fail")
        .to_string();
    let debug = format!("{:?}", result.expect_err("file init should fail"));
    assert!(!display.contains(PATH_SENTINEL));
    assert!(!debug.contains(PATH_SENTINEL));
    LogUtils::init(LogConfig::default()).expect("valid init after file error");
    assert!(LogUtils::is_initialized());
}

#[cfg(windows)]
fn non_utf8_case() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    let invalid_basename = OsString::from_wide(&[0xD800]);
    let result = LogUtils::init(
        LogConfig::new()
            .with_stdout(false)
            .with_file(LogFileConfig::new(PathBuf::from(invalid_basename))),
    );
    assert!(matches!(&result, Err(LogError::InvalidPath)));
    assert!(!LogUtils::is_initialized());
    LogUtils::init(LogConfig::default()).expect("valid init after non-UTF-8 path");
    assert!(LogUtils::is_initialized());
}

fn repeat_case(root: &Path) {
    fs::create_dir_all(root).expect("create repeat root");
    let candidate = root.join(PATH_SENTINEL);
    LogUtils::init(LogConfig::default()).expect("first init");
    assert!(matches!(
        LogUtils::init(
            LogConfig::new()
                .with_stdout(false)
                .with_file(LogFileConfig::new(&candidate)),
        ),
        Err(LogError::AlreadyInitialized)
    ));
    assert!(!candidate.exists());
}

fn concurrency_case() {
    let barrier = Arc::new(Barrier::new(8));
    let results = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..8 {
            let barrier = Arc::clone(&barrier);
            handles.push(scope.spawn(move || {
                barrier.wait();
                LogUtils::init(LogConfig::default())
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().expect("log init thread"))
            .collect::<Vec<_>>()
    });
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(LogError::AlreadyInitialized)))
            .count(),
        7
    );
}

fn level_case() {
    LogUtils::init(LogConfig::new().with_level(LogLevel::Error)).expect("level init");
    tracing::info!(target: "axutils::test", message = INFO_SENTINEL);
    tracing::error!(target: "axutils::test", message = ERROR_SENTINEL);
}

fn unique_temp_dir(case: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("axutils-log-global-{case}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create child root");
    path
}

fn count_occurrences(text: &str, needle: &str) -> usize {
    text.match_indices(needle).count()
}

fn assert_rotation_filename(case: &str, file_name: &str) {
    let suffix = file_name
        .strip_prefix("rotation.log.")
        .expect("rotation file should preserve the configured prefix");
    let expected_len = match case {
        "rotation-minutely" => 16,
        "rotation-hourly" => 13,
        "rotation-daily" => 10,
        _ => unreachable!("not a rotation case"),
    };
    assert_eq!(suffix.len(), expected_len, "unexpected rotation file name");
    assert!(
        suffix.bytes().enumerate().all(|(index, byte)| match index {
            4 | 7 | 10 | 13 => byte == b'-',
            _ => byte.is_ascii_digit(),
        }),
        "unexpected rotation timestamp suffix: {suffix}"
    );
}
