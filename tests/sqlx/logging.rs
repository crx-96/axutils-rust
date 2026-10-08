use std::io::Write;
use std::sync::{Arc, Mutex};

use axutils::sqlx::{SqlxClient, SqlxConfig};
use tracing::{subscriber, Level};
use tracing_subscriber::fmt::writer::MakeWriter;

#[derive(Clone)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
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

#[tokio::test]
async fn upstream_statement_logs_do_not_expose_sql_literals() {
    let capture = Arc::new(Mutex::new(Vec::new()));
    subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_ansi(false)
            .with_max_level(Level::TRACE)
            .with_writer(Capture(Arc::clone(&capture)))
            .finish(),
    )
    .unwrap();
    let client = SqlxClient::connect(SqlxConfig::new("sqlite::memory:").unwrap())
        .await
        .unwrap();
    let value = client
        .fetch_scalar_async(client.query_scalar::<String>("SELECT 'AXUTILS_SQL_LITERAL_SECRET'"))
        .await
        .unwrap();
    assert_eq!(value, "AXUTILS_SQL_LITERAL_SECRET");
    client.close_async().await.unwrap();
    let output = String::from_utf8(capture.lock().unwrap().clone()).unwrap();
    assert!(
        !output.contains("AXUTILS_SQL_LITERAL_SECRET"),
        "SQL 字面量泄漏: {output}"
    );
}
