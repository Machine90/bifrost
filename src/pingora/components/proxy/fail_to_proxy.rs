use http::header;
use pingora::{
    ErrorSource, ErrorType,
    http::ResponseHeader,
    proxy::{FailToProxy, Session},
};
use serde_json::json;

use crate::pingora::components::proxy::http_proxy::{HttpProxy, ProxyContext};

impl HttpProxy {
    pub(super) async fn fail_to_proxy_internal(
        &self,
        session: &mut Session,
        e: &pingora::Error,
        _ctx: &mut ProxyContext,
    ) -> FailToProxy {
        let status_code = match e.etype() {
            ErrorType::HTTPStatus(code) => *code,
            _ => match e.esource() {
                ErrorSource::Upstream => 502,
                ErrorSource::Downstream => match e.etype() {
                    ErrorType::WriteError | ErrorType::ReadError | ErrorType::ConnectionClosed => {
                        return FailToProxy {
                            error_code: 0,
                            can_reuse_downstream: false,
                        };
                    }
                    _ => 400,
                },
                ErrorSource::Internal | ErrorSource::Unset => 500,
            },
        };

        let detail = if let Some(context) = e.context.as_ref() {
            if status_code >= 500 {
                "Our engineers have been notified. Please try again later."
            } else {
                context.as_str()
            }
        } else {
            default_status_message(status_code)
        };

        let error_response = json!({
            "status": status_code,
            "message": detail
        })
        .to_string();

        let mut resp = match ResponseHeader::build(status_code, None) {
            Ok(resp) => resp,
            Err(_) => {
                return FailToProxy {
                    error_code: status_code,
                    can_reuse_downstream: false,
                };
            }
        };
        if let Err(e) = resp.insert_header(header::CONTENT_TYPE, "application/json") {
            tracing::error!(error = ?e, "Failed to insert header content-type");
            return FailToProxy {
                error_code: status_code,
                can_reuse_downstream: false,
            };
        }
        if let Err(e) = resp.insert_header(header::CONTENT_LENGTH, error_response.len()) {
            tracing::error!(error = ?e, "Failed to insert header content-length");
            return FailToProxy {
                error_code: status_code,
                can_reuse_downstream: false,
            };
        }

        if let Err(e) = session.write_response_header(Box::new(resp), false).await {
            tracing::error!(error = ?e, "Failed to write response header");
            return FailToProxy {
                error_code: status_code,
                can_reuse_downstream: false,
            };
        }

        if let Err(e) = session
            .write_response_body(Some(bytes::Bytes::from(error_response)), true)
            .await
        {
            tracing::error!(error = ?e, "Failed to write response body");
            return FailToProxy {
                error_code: status_code,
                can_reuse_downstream: false,
            };
        }

        FailToProxy {
            error_code: status_code,
            can_reuse_downstream: true,
        }
    }
}

const fn default_status_message(status_code: u16) -> &'static str {
    let default_message = match status_code {
        400 => "Bad Request - The server could not understand the request",
        401 => "Unauthorized - Authentication is required",
        403 => "Forbidden - You don't have permission to access this resource",
        404 => "Not Found - The requested resource does not exist",
        405 => "Method Not Allowed - The HTTP method is not supported",
        429 => "Too Many Requests - Rate limit exceeded, please try again later",
        500 => "Internal Server Error - Something went wrong on our end",
        502 => "Bad Gateway - The upstream server returned an invalid response",
        503 => "Service Unavailable - The service is temporarily unavailable",
        504 => "Gateway Timeout - The upstream server timed out",
        _ if status_code >= 500 => "Server Error - Please try again later",
        _ => "Client Error - Please check your request",
    };
    default_message
}
