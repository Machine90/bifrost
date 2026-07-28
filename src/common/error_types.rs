#![allow(unused)]

use std::str::FromStr;

use enum_as_inner::EnumAsInner;
use strum_macros::{Display, EnumString};

#[derive(Debug, Default, Display, EnumString, EnumAsInner)]
pub enum ErrorKind {
    #[default]
    InternalError,
    BadInput,
    Unauthorized,
    Forbidden,
    NotFound,
    TooManyRequests,
    Locked,
    ServiceUnavailable,
    Conflict,
}

pub trait ErrorTypes {
    /// Get the outer error kind from exceptions.
    fn kind(&self) -> ErrorKind;

    /// Check from all exceptions if contains specified error.
    fn contains_error(&self, kind: ErrorKind) -> bool;
}

impl ErrorTypes for &anyhow::Error {
    fn kind(&self) -> ErrorKind {
        let mut err_msg = self.to_string();
        if let Ok(kind) = ErrorKind::from_str(&err_msg) {
            return kind;
        }
        let mut source = self.source();
        while let Some(inner) = source {
            err_msg = inner.to_string();
            source = inner.source();
            if let Ok(kind) = ErrorKind::from_str(&err_msg) {
                return kind;
            }
        }
        ErrorKind::InternalError
    }

    fn contains_error(&self, kind: ErrorKind) -> bool {
        let kind_str = kind.to_string();

        let mut err_msg = self.to_string();
        if kind_str.eq(&err_msg) {
            return true;
        }
        let mut source = self.source();
        while let Some(inner) = source {
            err_msg = inner.to_string();
            source = inner.source();
            if kind_str.eq(&err_msg) {
                return true;
            }
        }
        false
    }
}

impl ErrorTypes for anyhow::Error {
    fn kind(&self) -> ErrorKind {
        (&self).kind()
    }

    fn contains_error(&self, kind: ErrorKind) -> bool {
        (&self).contains_error(kind)
    }
}
