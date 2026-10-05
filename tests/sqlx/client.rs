#![cfg(any(feature = "sqlx", feature = "sqlx-sqlite"))]

use std::time::Duration;

use axutils::sqlx::{SqlxClient, SqlxConfig, SqlxError, SqlxTransportErrorKind};
use axutils::utils::SqlxUtils;
use futures_util::future;
use tokio::{task as tokio_task, time as tokio_time};

#[test]
fn config_is_local_bounded_and_redacted() {
    let config = SqlxConfig::new("sqlite::memory:")
        .expect("SQLite URL should parse")
        .with_max_connections(1)
        .expect("memory SQLite accepts one connection")
        .with_max_rows(2)
        .expect("row limit should be configurable");
    let debug = format!("{config:?}");

    assert!(debug.contains("Sqlite"));
    assert!(!debug.contains("sqlite::memory:"));
    assert!(matches!(
        SqlxConfig::new("sqlite::memory:")
            .unwrap()
            .with_max_connections(2),
        Err(SqlxError::InvalidConfig {
            field: "max_connections"
        })
    ));
    assert!(matches!(
        SqlxConfig::new("postgres://user:secret@localhost/db?sslmode=require"),
        Err(SqlxError::InvalidConfig { field: "tls" })
    ));
}

#[test]
fn remote_scheme_construction_is_offline_and_redacted() {
    // 这里只验证 SqlxConfig 的本地解析；只有 SqlxClient::connect 才允许触发远端 I/O。
    for (url, driver) in [
        (
            "postgres://user:secret@example.invalid/db?sslmode=disable&application_name=hidden-query",
            "PostgreSql",
        ),
        (
            "postgresql://user:secret@example.invalid/db?sslmode=disable",
            "PostgreSql",
        ),
        (
            "mysql://user:secret@example.invalid/db",
            "MySql",
        ),
        (
            "mariadb://user:secret@example.invalid/db",
            "MySql",
        ),
    ] {
        let config = SqlxConfig::new(url).expect("supported URL scheme should parse locally");
        let debug = format!("{config:?}");
        assert!(debug.contains(driver), "Debug should identify the driver: {debug}");
        assert!(!debug.contains(url));
        for secret in ["user", "secret", "example.invalid", "hidden-query"] {
            assert!(!debug.contains(secret), "Debug leaked `{secret}`: {debug}");
        }
    }
}

#[tokio::test]
async fn client_covers_query_families_limits_errors_and_close() {
    let config = SqlxConfig::new("sqlite::memory:")
        .unwrap()
        .with_max_rows(2)
        .unwrap();
    let client = SqlxClient::connect(config).await.unwrap();

    client
        .execute_async(client.query("CREATE TABLE items (id INTEGER NOT NULL, name TEXT NOT NULL)"))
        .await
        .unwrap();
    for (id, name) in [(1_i64, "one"), (2, "two"), (3, "three")] {
        client
            .execute_async(
                client
                    .query("INSERT INTO items (id, name) VALUES (?, ?)")
                    .bind(id)
                    .bind(name),
            )
            .await
            .unwrap();
    }

    let scalar = client
        .fetch_scalar_async(client.query_scalar::<i64>("SELECT COUNT(*) FROM items"))
        .await
        .unwrap();
    assert_eq!(scalar, 3);

    let mapped: (i64, String) = client
        .fetch_one_as_async(
            client
                .query_as::<(i64, String)>("SELECT id, name FROM items WHERE id = ?")
                .bind(1_i64),
        )
        .await
        .unwrap();
    assert_eq!(mapped, (1, "one".to_owned()));

    assert!(client
        .fetch_optional_as_async::<(i64,)>(client.query_as("SELECT id FROM items WHERE id = 99"))
        .await
        .unwrap()
        .is_none());
    assert!(client
        .fetch_optional_async(client.query("SELECT id FROM items WHERE id = 99"))
        .await
        .unwrap()
        .is_none());
    assert!(matches!(
        client
            .fetch_one_async(client.query("SELECT id FROM items WHERE id = 99"))
            .await,
        Err(SqlxError::RowNotFound)
    ));

    let empty = client
        .fetch_all_as_async::<(i64,)>(client.query_as("SELECT id FROM items WHERE id = 99"))
        .await
        .unwrap();
    assert!(empty.is_empty());

    let exact = client
        .fetch_all_as_async::<(i64,)>(
            client
                .query_as("SELECT id FROM items WHERE id <= ? ORDER BY id")
                .bind(2_i64),
        )
        .await
        .unwrap();
    assert_eq!(exact, vec![(1,), (2,)]);

    assert!(matches!(
        client
            .fetch_all_async(client.query("SELECT id FROM items ORDER BY id"))
            .await,
        Err(SqlxError::RowLimitExceeded { limit: 2 })
    ));
    assert!(matches!(
        client
            .fetch_all_as_async::<(i64,)>(client.query_as(
                r#"
                    SELECT value FROM (
                        SELECT 1 AS position, 1 AS value
                        UNION ALL SELECT 2, 2
                        UNION ALL SELECT 3, 'bad'
                    ) ORDER BY position
                "#,
            ))
            .await,
        Err(SqlxError::Transport(SqlxTransportErrorKind::Decode))
    ));
    assert!(matches!(
        client
            .fetch_all_as_async::<(i64,)>(client.query_as(
                r#"
                    SELECT value FROM (
                        SELECT 1 AS position, 1 AS value
                        UNION ALL SELECT 2, 'bad'
                    ) ORDER BY position
                "#,
            ))
            .await,
        Err(SqlxError::Transport(SqlxTransportErrorKind::Decode))
    ));
    assert!(matches!(
        client
            .execute_async(client.query("SELECT definitely_missing FROM items"))
            .await,
        Err(SqlxError::Transport(_))
    ));

    assert!(matches!(
        client
            .fetch_all_async(client.query("SELECT definitely_missing FROM items"))
            .await,
        Err(SqlxError::Transport(_))
    ));
    assert!(matches!(
        client
            .fetch_scalar_async(client.query_scalar::<i64>("SELECT id FROM items WHERE id = 99",))
            .await,
        Err(SqlxError::RowNotFound)
    ));

    let mut transaction = client.begin_async().await.unwrap();
    client
        .query("INSERT INTO items (id, name) VALUES (?, ?)")
        .bind(4_i64)
        .bind("rollback")
        .execute(&mut *transaction)
        .await
        .unwrap();
    transaction.rollback().await.unwrap();

    let mut transaction = client.begin_async().await.unwrap();
    client
        .query("INSERT INTO items (id, name) VALUES (?, ?)")
        .bind(4_i64)
        .bind("commit")
        .execute(&mut *transaction)
        .await
        .unwrap();
    transaction.commit().await.unwrap();
    assert_eq!(
        client
            .fetch_scalar_async(client.query_scalar::<i64>("SELECT COUNT(*) FROM items"))
            .await
            .unwrap(),
        4
    );

    {
        let mut transaction = client.begin_async().await.unwrap();
        client
            .query("INSERT INTO items (id, name) VALUES (?, ?)")
            .bind(5_i64)
            .bind("drop")
            .execute(&mut *transaction)
            .await
            .unwrap();
    }
    assert_eq!(
        client
            .fetch_scalar_async(client.query_scalar::<i64>("SELECT COUNT(*) FROM items"))
            .await
            .unwrap(),
        4
    );

    assert!(!client.is_closed());
    client.close_async().await.unwrap();
    assert!(client.is_closed());
    assert!(matches!(
        client.execute_async(client.query("SELECT 1")).await,
        Err(SqlxError::PoolClosed)
    ));
}

#[tokio::test]
async fn pool_acquire_timeout_and_cancellation_release_connections() {
    let timeout_client = SqlxClient::connect(
        SqlxConfig::new("sqlite::memory:")
            .unwrap()
            // 连接池初始化本身也受 acquire timeout 约束；1ms 在高负载 CI 上可能先让
            // 初始 SQLite 连接超时，无法到达本测试真正要验证的“池已占满”路径。
            .with_acquire_timeout(Duration::from_millis(100))
            .unwrap(),
    )
    .await
    .unwrap();
    let held = timeout_client.begin_async().await.unwrap();
    assert!(matches!(
        timeout_client
            .execute_async(timeout_client.query("SELECT 1"))
            .await,
        Err(SqlxError::PoolAcquireTimeout)
    ));
    held.rollback().await.unwrap();

    let client = SqlxClient::connect(
        SqlxConfig::new("sqlite::memory:")
            .unwrap()
            .with_acquire_timeout(Duration::from_secs(5))
            .unwrap(),
    )
    .await
    .unwrap();
    let held = client.begin_async().await.unwrap();
    let task = tokio::spawn({
        let client = client.clone();
        async move { client.fetch_all_async(client.query("SELECT 1")).await }
    });
    tokio_task::yield_now().await;
    tokio_time::sleep(Duration::from_millis(10)).await;
    task.abort();
    match task.await {
        Err(error) => assert!(error.is_cancelled()),
        Ok(_) => panic!("cancelled fetch_all task unexpectedly completed"),
    }
    held.rollback().await.unwrap();

    assert_eq!(
        client
            .fetch_scalar_async(client.query_scalar::<i64>("SELECT 1"))
            .await
            .unwrap(),
        1
    );
    client.close_async().await.unwrap();
    timeout_client.close_async().await.unwrap();
}

#[tokio::test]
async fn global_utils_has_one_lifecycle_and_exposes_the_client() {
    assert!(!SqlxUtils::is_initialized());
    assert!(matches!(
        SqlxUtils::client(),
        Err(SqlxError::NotInitialized)
    ));

    let failed = SqlxUtils::init_async(
        SqlxConfig::new("sqlite://?vfs=axutils_definitely_missing_vfs").unwrap(),
    )
    .await;
    assert!(failed.is_err());
    assert!(!SqlxUtils::is_initialized());

    let handles = (0..8)
        .map(|_| {
            tokio::spawn(async {
                SqlxUtils::init_async(SqlxConfig::new("sqlite::memory:").unwrap()).await
            })
        })
        .collect::<Vec<_>>();
    let results = future::join_all(handles)
        .await
        .into_iter()
        .map(|result| result.expect("initialization task should not panic"))
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(SqlxError::AlreadyInitialized)))
            .count(),
        7
    );
    assert!(SqlxUtils::is_initialized());
    assert!(matches!(
        SqlxUtils::init_async(SqlxConfig::new("sqlite::memory:").unwrap()).await,
        Err(SqlxError::AlreadyInitialized)
    ));
    let client = SqlxUtils::client().unwrap();
    assert_eq!(
        client
            .fetch_scalar_async(client.query_scalar::<i64>("SELECT 1"))
            .await
            .unwrap(),
        1
    );

    client.close_async().await.unwrap();
    assert!(SqlxUtils::is_initialized());
    assert!(matches!(
        client.execute_async(client.query("SELECT 1")).await,
        Err(SqlxError::PoolClosed)
    ));
}

#[test]
fn public_types_are_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<SqlxClient>();
    assert_send_sync::<SqlxConfig>();
    assert_send_sync::<SqlxUtils>();
}

#[test]
fn async_connect_rejects_missing_runtime_before_io() {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    let mut future = Box::pin(SqlxClient::connect(
        SqlxConfig::new("sqlite::memory:").unwrap(),
    ));
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    assert!(matches!(
        future.as_mut().poll(&mut context),
        Poll::Ready(Err(SqlxError::RuntimeRequired))
    ));
}
