use std::borrow::Cow;
use std::error::Error as StdError;
use std::fmt;
use std::io;

use axutils::sqlx as axutils_sqlx;
use sqlx::error::{DatabaseError, ErrorKind};
use sqlx::Error as BackendError;

#[derive(Debug)]
struct CodedDatabaseError {
    code: Option<&'static str>,
    owned: bool,
}

impl fmt::Display for CodedDatabaseError {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("the classifier must not format database errors")
    }
}

impl StdError for CodedDatabaseError {}

impl DatabaseError for CodedDatabaseError {
    fn message(&self) -> &str {
        panic!("the classifier must not read database messages")
    }

    fn code(&self) -> Option<Cow<'_, str>> {
        self.code.map(|code| {
            if self.owned {
                Cow::Owned(code.to_owned())
            } else {
                Cow::Borrowed(code)
            }
        })
    }

    fn as_error(&self) -> &(dyn StdError + Send + Sync + 'static) {
        self
    }

    fn as_error_mut(&mut self) -> &mut (dyn StdError + Send + Sync + 'static) {
        self
    }

    fn into_error(self: Box<Self>) -> Box<dyn StdError + Send + Sync + 'static> {
        self
    }

    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}

fn database_error(code: Option<&'static str>, owned: bool) -> BackendError {
    BackendError::Database(Box::new(CodedDatabaseError { code, owned }))
}

#[test]
fn transaction_conflicts_match_only_exact_supported_sqlstates() {
    for owned in [false, true] {
        for code in ["40001", "40P01"] {
            let error = database_error(Some(code), owned);
            assert!(axutils_sqlx::is_postgres_transaction_conflict(&error));
            assert_eq!(
                error.as_database_error().unwrap().code().as_deref(),
                Some(code)
            );
        }

        for code in [
            None,
            Some(""),
            Some("23505"),
            Some("23503"),
            Some("02000"),
            Some("08006"),
            Some("40000"),
            Some("40p01"),
            Some("40001 extra"),
            Some(" 40001"),
        ] {
            assert!(!axutils_sqlx::is_postgres_transaction_conflict(
                &database_error(code, owned)
            ));
        }
    }
}

#[test]
fn non_database_errors_are_not_transaction_conflicts() {
    for error in [
        BackendError::RowNotFound,
        BackendError::PoolTimedOut,
        BackendError::PoolClosed,
        BackendError::BeginFailed,
        BackendError::InvalidSavePointStatement,
        BackendError::Io(io::Error::from(io::ErrorKind::TimedOut)),
        BackendError::Decode(Box::new(io::Error::from(io::ErrorKind::InvalidData))),
        BackendError::Protocol("40001".to_owned()),
    ] {
        assert!(!axutils_sqlx::is_postgres_transaction_conflict(&error));
    }
}
