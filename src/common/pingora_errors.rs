#![allow(unused)]

use http::StatusCode;
use pingora::{Error, ErrorType};

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
