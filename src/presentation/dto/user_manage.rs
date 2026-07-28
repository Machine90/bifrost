use std::collections::{HashMap, HashSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddUserConfigRequest {
    pub user_id: String,
    #[serde(
        default,
        deserialize_with = "crate::common::serde_helper::deserialize_option_str"
    )]
    pub platform: Option<String>,
    pub roles: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddUserConfigResponse {
    pub config_id: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EditUserConfigRequest {
    pub user_id: String,
    #[serde(
        default,
        deserialize_with = "crate::common::serde_helper::deserialize_option_str"
    )]
    pub platform: Option<String>,
    pub roles: Option<HashSet<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EditUserConfigResponse {
    pub result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteUserConfigRequest {
    pub config_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteUserConfigResponse {
    pub result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetCurrentUserResponse {
    pub user_id: Option<String>,
    #[serde(rename = "roles")]
    pub platform_roles: HashMap<String, HashSet<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserConfigDto {
    pub id: i32,
    pub user_id: String,
    pub platform: String,
    pub roles: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListUserConfigsByIdsRequest {
    pub user_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListUserConfigsByIdsResponse {
    pub user_configs: Vec<UserConfigDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListUserPrivilegeRequest {
    #[serde(
        default,
        deserialize_with = "crate::common::serde_helper::deserialize_option_str"
    )]
    pub platform: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserPrivilegeDto {
    pub service: String,
    pub method: String,
    pub url_path: String,
    pub key: Option<String>,
    pub associated_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListUserPrivilegeResponse {
    pub user_privileges: Vec<UserPrivilegeDto>,
}
