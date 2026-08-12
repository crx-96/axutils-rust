#[cfg(feature = "sqlx-tokio")]
fn compile_api() {
    use axutils::sqlx::{
        SqlxClient as ModuleClient, SqlxConfig as ModuleConfig, SqlxError as ModuleError,
        SqlxQueryResult as ModuleQueryResult, SqlxRow as ModuleRow,
        SqlxTransaction as ModuleTransaction,
    };
    use axutils::utils::sqlx_utils::SqlxUtils as NestedUtils;
    use axutils::utils::SqlxUtils as UtilsClient;
    use axutils::{
        SqlxClient, SqlxConfig, SqlxQueryResult, SqlxRow, SqlxTransaction, SqlxUtils,
    };

    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<SqlxClient>();
    assert_send_sync::<SqlxConfig>();
    assert_send_sync::<SqlxUtils>();

    let _: Option<ModuleClient> = None;
    let _: Option<ModuleConfig> = None;
    let _: Option<ModuleError> = None;
    let _: Option<ModuleQueryResult> = None;
    let _: Option<ModuleRow> = None;
    let _: Option<ModuleTransaction<'static>> = None;
    let _: Option<SqlxQueryResult> = None;
    let _: Option<SqlxRow> = None;
    let _: Option<SqlxTransaction<'static>> = None;
    let _: Option<UtilsClient> = None;
    let _: Option<NestedUtils> = None;

    let config = SqlxConfig::new("sqlite::memory:").expect("fixture config");
    let _ = config.with_max_rows(8).expect("fixture row limit");
    let _ = SqlxClient::connect;
    let _ = SqlxClient::query;
    let _ = SqlxClient::query_as::<(i64,)>;
    let _ = SqlxClient::query_scalar::<i64>;
    let _ = SqlxClient::execute_async;
    let _ = SqlxClient::fetch_one_async;
    let _ = SqlxClient::fetch_one_as_async::<(i64,)>;
    let _ = SqlxClient::fetch_optional_async;
    let _ = SqlxClient::fetch_optional_as_async::<(i64,)>;
    let _ = SqlxClient::fetch_all_async;
    let _ = SqlxClient::fetch_all_as_async::<(i64,)>;
    let _ = SqlxClient::fetch_scalar_async::<i64>;
    let _ = SqlxClient::begin_async;
    let _ = SqlxClient::close_async;
    let _ = SqlxClient::is_closed;

    let _ = SqlxUtils::init;
    let _ = SqlxUtils::is_initialized;
    let _ = SqlxUtils::query;
    let _ = SqlxUtils::query_as::<(i64,)>;
    let _ = SqlxUtils::query_scalar::<i64>;
    let _ = SqlxUtils::execute_async;
    let _ = SqlxUtils::fetch_one_async;
    let _ = SqlxUtils::fetch_one_as_async::<(i64,)>;
    let _ = SqlxUtils::fetch_optional_async;
    let _ = SqlxUtils::fetch_optional_as_async::<(i64,)>;
    let _ = SqlxUtils::fetch_all_async;
    let _ = SqlxUtils::fetch_all_as_async::<(i64,)>;
    let _ = SqlxUtils::fetch_scalar_async::<i64>;
    let _ = SqlxUtils::begin_async;
    let _ = SqlxUtils::close_async;

    let _ = compile_async_api;
}

#[cfg(feature = "sqlx-tokio")]
async fn compile_async_api(
    client: &axutils::SqlxClient,
) -> Result<(), Box<dyn std::error::Error>> {
    let query = client.query("SELECT ?").bind(1_i64);
    let _ = client.execute_async(query).await;

    let mut builder = sqlx::QueryBuilder::<sqlx::Any>::new("SELECT ");
    builder.push_bind(1_i64);
    let _ = client.execute_async(builder.build()).await;

    let mut transaction = client.begin_async().await?;
    sqlx::query::<sqlx::Any>("SELECT 1")
        .execute(&mut *transaction)
        .await?;
    transaction.rollback().await?;
    Ok(())
}

#[cfg(feature = "sqlx-tokio")]
fn main() {
    compile_api();
}

#[cfg(any(feature = "none", feature = "tokio-only", feature = "sqlx-only"))]
fn main() {}

#[cfg(feature = "negative-no-sqlx-module")]
fn main() {
    let _ = axutils::sqlx::SqlxClient::connect;
}

#[cfg(feature = "negative-no-sqlx-root")]
fn main() {
    let _ = axutils::SqlxClient::connect;
}

#[cfg(feature = "negative-no-sqlx-utils")]
fn main() {
    let _ = axutils::SqlxUtils::is_initialized;
}

#[cfg(feature = "negative-sqlx-only-module")]
fn main() {
    let _ = axutils::sqlx::SqlxClient::connect;
}

#[cfg(feature = "negative-sqlx-only-root")]
fn main() {
    let _ = axutils::SqlxClient::connect;
}

#[cfg(feature = "negative-sqlx-only-utils")]
fn main() {
    let _ = axutils::SqlxUtils::is_initialized;
}

#[cfg(feature = "negative-sqlx-only-async")]
fn main() {
    let _ = axutils::SqlxClient::connect;
}

#[cfg(feature = "negative-tokio-module")]
fn main() {
    let _ = axutils::sqlx::SqlxClient::connect;
}

#[cfg(feature = "negative-tokio-root")]
fn main() {
    let _ = axutils::SqlxClient::connect;
}

#[cfg(feature = "negative-tokio-utils")]
fn main() {
    let _ = axutils::SqlxUtils::is_initialized;
}

#[cfg(feature = "negative-tokio-async")]
fn main() {
    let _ = axutils::SqlxUtils::execute_async;
}

#[cfg(not(any(
    feature = "sqlx-tokio",
    feature = "none",
    feature = "tokio-only",
    feature = "sqlx-only",
    feature = "negative-no-sqlx-module",
    feature = "negative-no-sqlx-root",
    feature = "negative-no-sqlx-utils",
    feature = "negative-sqlx-only-module",
    feature = "negative-sqlx-only-root",
    feature = "negative-sqlx-only-utils",
    feature = "negative-sqlx-only-async",
    feature = "negative-tokio-module",
    feature = "negative-tokio-root",
    feature = "negative-tokio-utils",
    feature = "negative-tokio-async"
)))]
fn main() {}
