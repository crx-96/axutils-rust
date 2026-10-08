#![cfg(any(
    feature = "sqlx-postgres",
    feature = "sqlx-mysql",
    feature = "sqlx-sqlite"
))]

#[cfg(feature = "sqlx-sqlite")]
#[path = "sqlx/client.rs"]
mod client;

#[cfg(feature = "sqlx-sqlite")]
#[path = "sqlx/logging.rs"]
mod logging;

#[path = "sqlx/errors.rs"]
mod errors;

#[cfg(feature = "sqlx-postgres")]
#[path = "sqlx/postgres.rs"]
mod postgres;
