#![allow(unused)]

use super::error_types::{ErrorKind, ErrorTypes as _};
use axum::http::StatusCode;

pub(crate) fn to_http_status(err: anyhow::Error) -> StatusCode {
    let err_kind = err.kind();
    match err_kind {
        ErrorKind::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        ErrorKind::BadInput => StatusCode::BAD_REQUEST,
        ErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
        ErrorKind::Forbidden => StatusCode::FORBIDDEN,
        ErrorKind::NotFound => StatusCode::NOT_FOUND,
        ErrorKind::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
        ErrorKind::Locked => StatusCode::LOCKED,
        ErrorKind::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        ErrorKind::Conflict => StatusCode::CONFLICT,
    }
}

pub(crate) fn to_http_error(err: anyhow::Error) -> (StatusCode, String) {
    let err_kind = err.kind();
    let status = match err_kind {
        ErrorKind::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        ErrorKind::BadInput => StatusCode::BAD_REQUEST,
        ErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
        ErrorKind::Forbidden => StatusCode::FORBIDDEN,
        ErrorKind::NotFound => StatusCode::NOT_FOUND,
        ErrorKind::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
        ErrorKind::Locked => StatusCode::LOCKED,
        ErrorKind::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        ErrorKind::Conflict => StatusCode::CONFLICT,
    };
    let err_msg = err.to_string();
    (status, err_msg)
}
