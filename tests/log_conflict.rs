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
    tracing::info!(target: "axutils::external", message = SENTINEL);
    let output = String::from_utf8(capture.lock().expect("capture lock").clone())
        .expect("UTF-8 captured output");
    assert_eq!(output.match_indices(SENTINEL).count(), 1);
}
