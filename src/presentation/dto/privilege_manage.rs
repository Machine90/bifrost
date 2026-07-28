use std::{collections::HashSet, str::FromStr};

use anyhow::Context;
use http::Method;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    common::{constants::http_proxy::LOCALHOST_SERVICE_NAME, error_types::ErrorKind},
    domain::model::{entity::backend_api_rule::BackendApiRule, value::role::Role},
};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BackendApi {
    /// Customized key of the API, e.g. "key": "getCurrentUser",
    #[serde(
        default,
        deserialize_with = "crate::common::serde_helper::deserialize_option_str"
    )]
    pub key: Option<String>,
    /// The backend API belongs to service
    #[serde(
        default,
        deserialize_with = "crate::common::serde_helper::deserialize_option_str"
    )]
    pub service: Option<String>,
    /// Method of target interface.
    pub method: String,
    /// Path regex of the API, could be a full path, e.g. "/api/v1/region/upload"
    pub url_path: String,
    /// Configured allowed roles of API, e.g. ["super", "manager"]
    pub roles: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddPrivilegeConfigRequest {
    pub config_key: String,
    pub backend_apis: Vec<BackendApi>,
    #[serde(
        default,
        deserialize_with = "crate::common::serde_helper::deserialize_option_str"
    )]
    pub platform: Option<String>,
    #[serde(default)]
    pub check_service_available: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddPrivilegeConfigResponse {
    pub id: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EditPrivilegeConfigRequest {
    pub id: i32,
    #[serde(
        default,
        deserialize_with = "crate::common::serde_helper::deserialize_option_str"
    )]
    pub config_key: Option<String>,
    pub backend_apis: Option<Vec<BackendApi>>,
    #[serde(
        default,
        deserialize_with = "crate::common::serde_helper::deserialize_option_str"
    )]
    pub platform: Option<String>,
    #[serde(default)]
    pub check_service_available: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EditPrivilegeConfigResponse {
    pub result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListPrivilegeConfigRequest {
    #[serde(
        default,
        deserialize_with = "crate::common::serde_helper::deserialize_option_str"
    )]
    pub platform: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PrivilegeConfig {
    pub id: i32,
    pub config_key: String,
    pub backend_apis: Vec<BackendApi>,
    pub allowed_roles: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListPrivilegeConfigResponse {
    pub data: Vec<PrivilegeConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeletePrivilegeConfigRequest {
    pub id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeletePrivilegeConfigResponse {
    pub result: String,
}

impl From<&BackendApiRule> for BackendApi {
    fn from(rule: &BackendApiRule) -> Self {
        let BackendApiRule {
            key,
            service,
            url_path,
            roles,
            method,
        } = rule;
        BackendApi {
            key: key.clone(),
            service: Some(service.to_string()),
            method: method.as_str().to_string(),
            url_path: url_path.to_string(),
            roles: roles.iter().map(Role::to_string).collect(),
        }
    }
}

impl TryInto<BackendApiRule> for BackendApi {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<BackendApiRule, Self::Error> {
        let Self {
            key,
            service,
            url_path,
            roles,
            method,
        } = self;
        Ok(BackendApiRule {
            key,
            // if service no specified, we default to use localhost as service.
            service: service.unwrap_or(LOCALHOST_SERVICE_NAME.to_string()),
            method: Method::from_str(&method.to_uppercase())
                .context(ErrorKind::BadInput)
                .context("Failed to convert method.")?,
            url_path,
            roles: roles.iter().map(Role::from_str).collect(),
        })
    }
}
