use std::collections::HashSet;

use http::Method;

use crate::domain::model::value::role::Role;

#[derive(Debug, Clone)]
pub struct BackendApiRule {
    pub key: Option<String>,
    pub service: String,
    pub method: Method,
    pub url_path: String,
    pub roles: HashSet<Role>,
}
