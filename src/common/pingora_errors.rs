#![allow(unused)]

use http::StatusCode;
use pingora::{Error, ErrorType};

use crate::common::error_types::{ErrorKind, ErrorTypes};

pub(crate) fn forbidden() -> Box<Error> {
    Error::new(ErrorType::HTTPStatus(StatusCode::FORBIDDEN.as_u16()))
}

pub(crate) fn internal_error() -> Box<Error> {
    Error::new(ErrorType::HTTPStatus(
        StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
    ))
}

pub(crate) fn notfound() -> Box<Error> {
    Error::new(ErrorType::HTTPStatus(StatusCode::NOT_FOUND.as_u16()))
}

pub(crate) fn to_pingora_error(err: anyhow::Error) -> Box<Error> {
    let err_kind = err.kind();
    match err_kind {
        ErrorKind::InternalError => Error::new(ErrorType::InternalError),
        ErrorKind::BadInput => Error::new(ErrorType::HTTPStatus(StatusCode::BAD_REQUEST.as_u16())),
        ErrorKind::Unauthorized => {
            Error::new(ErrorType::HTTPStatus(StatusCode::UNAUTHORIZED.as_u16()))
        }
        ErrorKind::Forbidden => Error::new(ErrorType::HTTPStatus(StatusCode::FORBIDDEN.as_u16())),
        ErrorKind::NotFound => Error::new(ErrorType::HTTPStatus(StatusCode::NOT_FOUND.as_u16())),
        ErrorKind::TooManyRequests => Error::new(ErrorType::HTTPStatus(
            StatusCode::TOO_MANY_REQUESTS.as_u16(),
        )),
        ErrorKind::Locked => Error::new(ErrorType::HTTPStatus(StatusCode::LOCKED.as_u16())),
        ErrorKind::ServiceUnavailable => Error::new(ErrorType::HTTPStatus(
            StatusCode::SERVICE_UNAVAILABLE.as_u16(),
        )),
        ErrorKind::Conflict => Error::new(ErrorType::HTTPStatus(StatusCode::CONFLICT.as_u16())),
    }
}
