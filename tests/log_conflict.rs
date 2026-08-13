#![cfg(feature = "logging")]

use std::io::Write;
use std::sync::{Arc, Mutex};

use axutils::{LogConfig, LogError, LogUtils};
use tracing_subscriber::fmt::writer::MakeWriter;

#[derive(Clone)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("capture lock")
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Capture {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[test]
fn external_global_subscriber_conflict_is_reported_without_consuming_state() {
    const SENTINEL: &str = "AXUTILS_EXTERNAL_SUBSCRIBER_SENTINEL";
    let capture = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(Capture(Arc::clone(&capture)))
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("install test subscriber");

    assert!(matches!(
        LogUtils::init(LogConfig::default()),
        Err(LogError::GlobalSubscriberAlreadySet)
    ));
    assert!(!LogUtils::is_initialized());
    tracing::info!(
        target: "axutils::external",
        operation = "after_first_conflict",
        outcome = "success",
        message = SENTINEL,
    );
    assert!(matches!(
        LogUtils::init(LogConfig::default()),
        Err(LogError::GlobalSubscriberAlreadySet)
    ));
    assert!(!LogUtils::is_initialized());
    tracing::info!(
        target: "axutils::external",
        operation = "after_second_conflict",
        outcome = "success",
        message = SENTINEL,
    );
    let output = String::from_utf8(capture.lock().expect("capture lock").clone())
        .expect("UTF-8 captured output");
    assert_external_event_count(&output, "after_first_conflict", SENTINEL, 1);
    assert_external_event_count(&output, "after_second_conflict", SENTINEL, 1);
    assert_eq!(output.match_indices(SENTINEL).count(), 2);
    assert!(!output.contains('\u{1b}'));
}

fn assert_external_event_count(output: &str, operation: &str, message: &str, expected: usize) {
    let count = output
        .lines()
        .filter(|line| {
            line.split_ascii_whitespace()
                .any(|field| field == "axutils::external:")
                && has_field(line, "operation", operation)
                && has_field(line, "outcome", "success")
                && line.contains(message)
        })
        .count();
    assert_eq!(
        count, expected,
        "expected external subscriber to capture {expected} {operation} event(s):\n{output}"
    );
}

fn has_field(line: &str, name: &str, value: &str) -> bool {
    line.contains(&format!(r#"{name}="{value}""#))
        || line.contains(&format!(r#"{name} = "{value}""#))
}
