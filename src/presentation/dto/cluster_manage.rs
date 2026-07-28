use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Request to query all online services of internal services
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListClusterServiceRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ClusterService {
    pub registry: String,
    pub name: String,
}

/// Response with all services name
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListClusterServiceResponse {
    pub service_names: Vec<ClusterService>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct BackendInfo {
    pub address: String,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListServiceBackendRequest {
    pub service_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListServiceBackendResponse {
    pub backends: Vec<BackendInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListClusterRolesRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RoleEntry {
    pub key: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListClusterRolesResponse {
    pub roles: Vec<RoleEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlatformEntry {
    pub key: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListClusterPlatformResponse {
    pub platforms: Vec<PlatformEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RenameRoleRequest {
    pub old_role: String,
    pub new_role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RenameRoleResponse {
    pub updated_privilege_config_count: usize,
    pub updated_user_config_count: usize,
}
