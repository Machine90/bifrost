use std::path::Path;

use http::Uri;
use pingora::proxy::Session;
use url::Url;

use crate::common::{
    constants::customized_headers, header_map_ext::GetIgnoreCase, pingora_errors::internal_error,
};

#[derive(Debug, Clone)]
pub enum ServiceExtractFrom {
    Header(String),
    Path(String),
    NoFound,
}

impl ServiceExtractFrom {
    pub fn extract_service_name(session: &Session) -> Self {
        // we allow requester set service name in header
        let service_name_from_header = session
            .req_header()
            .headers
            .get_ignore_case(customized_headers::HEADER_SERVICE_KEY)
            .map(|value| value.to_str().ok())
            .flatten();

        if let Some(service) = service_name_from_header {
            return Self::Header(service.to_string());
        }

        let uri_path = session.req_header().uri.path();
        let mut first_path = None;
        // try to parse first path segment from url
        if let Some(addr) = session.server_addr() {
            let url = format!("{}{uri_path}", addr.to_string());
            first_path = Url::parse(&url)
                .ok()
                .map(|url| {
                    url.path_segments().map(|paths| {
                        paths
                            .into_iter()
                            .filter(|v| !v.is_empty())
                            .map(|v| v.to_string())
                            .next()
                    })
                })
                .flatten()
                .flatten();
        }

        let service_name_from_path = first_path.or_else(|| {
            // otherwise path uri as path and try to acquire first path segment
            // from it.
            let uri_path = Path::new(&uri_path);
            let first_path = uri_path
                .iter()
                .filter(|p| !p.is_empty())
                .filter(|p| !matches!(p.to_str(), Some("/")))
                .next()
                .map(|p| p.to_string_lossy().to_string());
            first_path
        });

        if let Some(service) = service_name_from_path {
            return Self::Path(service.to_string());
        }
        Self::NoFound
    }

    pub fn get_service_name(&self) -> Option<&str> {
        match self {
            ServiceExtractFrom::Header(n) => Some(n.as_str()),
            ServiceExtractFrom::Path(n) => Some(n.as_str()),
            ServiceExtractFrom::NoFound => None,
        }
    }

    pub fn remove_service_name_from_uri(&self, session: &mut Session) -> pingora::Result<()> {
        let service_name = match self {
            // only remove the name extracted from Uri
            ServiceExtractFrom::Path(service_name) => service_name.as_str(),
            ServiceExtractFrom::Header(_) | ServiceExtractFrom::NoFound => return Ok(()),
        };

        let original_uri = &session.req_header().uri;
        let path_and_query = session
            .req_header()
            .uri
            .path_and_query()
            .cloned()
            .ok_or(internal_error())?;
        let service_path_prefix = format!("/{service_name}");
        let uri_path = path_and_query.path();
        let uri_queries = path_and_query.query();

        if uri_path.starts_with(&service_path_prefix) {
            if let Some(real_uri_path) = uri_path.strip_prefix(&service_path_prefix) {
                let new_path_and_query = match uri_queries {
                    Some(queries) => format!("{real_uri_path}?{queries}"),
                    None => real_uri_path.to_string(),
                };
                let mut new_uri_builder = Uri::builder().path_and_query(new_path_and_query);
                if let Some(auth) = original_uri.authority() {
                    new_uri_builder = new_uri_builder.authority(auth.clone());
                }
                if let Some(schema) = original_uri.scheme() {
                    new_uri_builder = new_uri_builder.scheme(schema.clone());
                }
                session
                    .as_mut()
                    .req_header_mut()
                    .set_uri(new_uri_builder.build().map_err(|_| internal_error())?);
            }
        }
        Ok(())
    }
}
