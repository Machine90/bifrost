use aide::axum::{
    ApiRouter,
    routing::{get, post},
};
use axum::{Extension, Json, extract::Query};
use http::StatusCode;
use unic_langid::LanguageIdentifier;

use crate::{
    application::factory,
    common::error_to_http_status::to_http_error,
    domain::model::{
        entity::{
            backend_api_rule::BackendApiRule,
            privilege_rule::{PrivilegeRule, PrivilegeRuleEdit},
        },
        value::{platform::Platform, role::Role},
    },
    presentation::dto::privilege_manage::{
        AddPrivilegeConfigRequest, AddPrivilegeConfigResponse, BackendApi,
        DeletePrivilegeConfigRequest, DeletePrivilegeConfigResponse, EditPrivilegeConfigRequest,
        EditPrivilegeConfigResponse, ListPrivilegeConfigRequest, ListPrivilegeConfigResponse,
        PrivilegeConfig,
    },
};

pub mod url {
    use constcat::concat;

    use crate::common::constants::http_server::BASE_URL;

    pub const PRIVILEGE_ADD: &str = concat!(BASE_URL, "/privilege/add");
    pub const PRIVILEGE_EDIT: &str = concat!(BASE_URL, "/privilege/edit");
    pub const PRIVILEGE_LIST: &str = concat!(BASE_URL, "/privilege/list");
    pub const PRIVILEGE_DEL: &str = concat!(BASE_URL, "/privilege/delete");
}

pub(crate) fn register(router: ApiRouter) -> ApiRouter {
    router
        .api_route(url::PRIVILEGE_ADD, post(add_privilege_config))
        .api_route(url::PRIVILEGE_EDIT, post(edit_privilege_config))
        .api_route(url::PRIVILEGE_LIST, get(list_privilege_configs))
        .api_route(url::PRIVILEGE_DEL, post(delete_privilege_config))
}

#[axum::debug_handler]
async fn add_privilege_config(
    Json(request): Json<AddPrivilegeConfigRequest>,
) -> Result<Json<AddPrivilegeConfigResponse>, (StatusCode, String)> {
    let AddPrivilegeConfigRequest {
        config_key,
        backend_apis,
        platform,
        check_service_available,
    } = request;
    let mut converted_backend_apis = vec![];
    for api in backend_apis {
        let api_rule: BackendApiRule = api.try_into().map_err(to_http_error)?;
        converted_backend_apis.push(api_rule);
    }
    let privilege = PrivilegeRule {
        config_key,
        backend_apis: converted_backend_apis,
        platform: platform
            .as_ref()
            .map(|s| Platform::from_str(s))
            .unwrap_or(Platform::Gateway),
    };
    let check_service_available = check_service_available.unwrap_or(true);
    let cluster_manage_svc = factory::get_cluster_manage_svc().await;
    let rule_id = cluster_manage_svc
        .add_privilege_config(privilege, check_service_available)
        .await
        .map_err(to_http_error)?;
    Ok(Json(AddPrivilegeConfigResponse { id: rule_id }))
}

#[axum::debug_handler]
async fn edit_privilege_config(
    Json(request): Json<EditPrivilegeConfigRequest>,
) -> Result<Json<EditPrivilegeConfigResponse>, (StatusCode, String)> {
    let EditPrivilegeConfigRequest {
        id,
        config_key,
        backend_apis,
        platform,
        check_service_available,
    } = request;
    let mut converted_backend_apis = vec![];
    for api in backend_apis.unwrap_or_default() {
        let api_rule: BackendApiRule = api.try_into().map_err(to_http_error)?;
        converted_backend_apis.push(api_rule);
    }
    let privilege = PrivilegeRuleEdit {
        config_key,
        backend_apis: (!converted_backend_apis.is_empty()).then_some(converted_backend_apis),
        platform: platform.map(|p| Platform::from_str(&p)),
    };
    let check_service_available = check_service_available.unwrap_or(true);
    let cluster_manage_svc = factory::get_cluster_manage_svc().await;
    cluster_manage_svc
        .edit_privilege_config(id, privilege, check_service_available)
        .await
        .map_err(to_http_error)?;
    Ok(Json(EditPrivilegeConfigResponse {
        result: "Ok".to_string(),
    }))
}

#[axum::debug_handler]
async fn delete_privilege_config(
    Json(request): Json<DeletePrivilegeConfigRequest>,
) -> Result<Json<DeletePrivilegeConfigResponse>, (StatusCode, String)> {
    let cluster_manage_svc = factory::get_cluster_manage_svc().await;
    cluster_manage_svc
        .delete_privilege_config(request.id)
        .await
        .map_err(to_http_error)?;
    Ok(Json(DeletePrivilegeConfigResponse {
        result: "Ok".to_string(),
    }))
}

#[axum::debug_handler]
async fn list_privilege_configs(
    Extension(_prefer_language): Extension<LanguageIdentifier>,
    Query(request): Query<ListPrivilegeConfigRequest>,
) -> Result<Json<ListPrivilegeConfigResponse>, StatusCode> {
    let ListPrivilegeConfigRequest { platform } = request;
    let platform = platform.map(|p| Platform::from_str(&p)).unwrap_or_default();
    let cluster_manage_svc = factory::get_cluster_manage_svc().await;
    let privileges = cluster_manage_svc
        .list_all_privilege_configs_of_platform(platform)
        .await
        .into_iter()
        .filter(|(_, p)| p.is_editable())
        .map(|(rule_id, privilege)| {
            let roles = privilege
                .allowed_roles()
                .iter()
                .map(Role::to_string)
                .collect();
            let backend_apis = privilege
                .as_ref()
                .backend_apis
                .iter()
                .map(BackendApi::from)
                .collect();
            PrivilegeConfig {
                id: rule_id,
                config_key: privilege.config_key.to_string(),
                backend_apis,
                allowed_roles: roles,
            }
        })
        .collect();
    Ok(Json(ListPrivilegeConfigResponse { data: privileges }))
}
