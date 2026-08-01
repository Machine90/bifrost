use aide::axum::{
    ApiRouter,
    routing::{get, post},
};
use axum::{Json, extract::Query};
use http::{HeaderMap, StatusCode};
use tracing::Level;

use crate::{
    application::factory,
    common::{
        error_to_http_status::to_http_error, sentry_ext::SentryError, tracing_ext::TracingResultExt,
    },
    domain::model::{
        entity::user_config::{UserConfigAdd, UserConfigEdit},
        value::{platform::Platform, role::Role},
    },
    presentation::dto::user_manage::{
        AddUserConfigRequest, AddUserConfigResponse, DeleteUserConfigRequest,
        DeleteUserConfigResponse, EditUserConfigRequest, EditUserConfigResponse,
        GetCurrentUserResponse, ListUserConfigsByIdsRequest, ListUserConfigsByIdsResponse,
        ListUserPrivilegeRequest, ListUserPrivilegeResponse, UserConfigDto, UserPrivilegeDto,
    },
};

pub mod url {
    use constcat::concat;

    use crate::common::constants::http_server::BASE_URL;

    pub const USER_CONFIG_CUR: &str = concat!(BASE_URL, "/user/current");
    pub const USER_CONFIG_ADD: &str = concat!(BASE_URL, "/user/add");
    pub const USER_CONFIG_EDIT: &str = concat!(BASE_URL, "/user/edit");
    pub const USER_CONFIG_DEL: &str = concat!(BASE_URL, "/user/delete");
    pub const USER_CONFIG_LIST: &str = concat!(BASE_URL, "/user/list");
    pub const USER_CONFIG_PRIVILEGES: &str = concat!(BASE_URL, "/user/privileges");
}

pub(crate) fn register(router: ApiRouter) -> ApiRouter {
    router
        .api_route(url::USER_CONFIG_CUR, get(get_current_user))
        .api_route(url::USER_CONFIG_ADD, post(add_user_config))
        .api_route(url::USER_CONFIG_EDIT, post(edit_user_config))
        .api_route(url::USER_CONFIG_DEL, post(del_user_config))
        .api_route(url::USER_CONFIG_LIST, post(list_user_configs_by_ids))
        .api_route(url::USER_CONFIG_PRIVILEGES, get(list_user_privileges))
}

#[axum::debug_handler]
async fn get_current_user(
    headers: HeaderMap,
) -> Result<Json<GetCurrentUserResponse>, (StatusCode, String)> {
    let user_privilege_app = factory::get_user_privilege_app().await;
    let (current_user_id, user_roles) = user_privilege_app
        .get_user_roles(headers)
        .await
        .log_if_error(Level::ERROR)
        .report_sentry()
        .map_err(to_http_error)?;
    Ok(Json(GetCurrentUserResponse {
        user_id: current_user_id,
        platform_roles: user_roles
            .into_iter()
            .map(|(platform, roles)| {
                (
                    platform.to_string(),
                    roles.iter().map(ToString::to_string).collect(),
                )
            })
            .collect(),
    }))
}

#[axum::debug_handler]
async fn add_user_config(
    Json(request): Json<AddUserConfigRequest>,
) -> Result<Json<AddUserConfigResponse>, (StatusCode, String)> {
    let user_config_add = UserConfigAdd {
        user_id: request.user_id,
        platform: request
            .platform
            .map(|s| Platform::from_str(&s))
            .unwrap_or(Platform::Gateway),
        roles: request.roles.iter().map(Role::from_str).collect(),
    };
    let user_manage_svc = factory::get_user_manage_svc().await;
    let config_id = user_manage_svc
        .add_new_user_config(user_config_add)
        .await
        .log_if_error(Level::ERROR)
        .report_sentry()
        .map_err(to_http_error)?;

    let response = AddUserConfigResponse { config_id };
    Ok(Json(response))
}

#[axum::debug_handler]
async fn edit_user_config(
    Json(request): Json<EditUserConfigRequest>,
) -> Result<Json<EditUserConfigResponse>, (StatusCode, String)> {
    let user_config_edit = UserConfigEdit {
        user_id: request.user_id,
        platform: request
            .platform
            .map(|s| Platform::from_str(&s))
            .unwrap_or(Platform::Gateway),
        roles: request
            .roles
            .map(|roles| roles.iter().map(Role::from_str).collect()),
    };
    let user_manage_svc = factory::get_user_manage_svc().await;
    user_manage_svc
        .edit_new_user_config(user_config_edit)
        .await
        .log_if_error(Level::ERROR)
        .report_sentry()
        .map_err(to_http_error)?;
    Ok(Json(EditUserConfigResponse {
        result: "Ok".to_owned(),
    }))
}

#[axum::debug_handler]
async fn del_user_config(
    Json(request): Json<DeleteUserConfigRequest>,
) -> Result<Json<DeleteUserConfigResponse>, (StatusCode, String)> {
    let user_manage_svc = factory::get_user_manage_svc().await;
    user_manage_svc
        .delete_user_config(request.config_id)
        .await
        .log_if_error(Level::ERROR)
        .report_sentry()
        .map_err(to_http_error)?;
    Ok(Json(DeleteUserConfigResponse {
        result: "Ok".to_owned(),
    }))
}

#[axum::debug_handler]
async fn list_user_configs_by_ids(
    Json(request): Json<ListUserConfigsByIdsRequest>,
) -> Result<Json<ListUserConfigsByIdsResponse>, (StatusCode, String)> {
    let user_manage_svc = factory::get_user_manage_svc().await;
    let configs = user_manage_svc
        .list_user_config_by_user_ids(request.user_ids)
        .await
        .log_if_error(Level::ERROR)
        .report_sentry()
        .map_err(to_http_error)?
        .into_iter()
        .map(|entity| UserConfigDto {
            id: entity.config_id,
            user_id: entity.user_id,
            platform: entity.platform.to_string(),
            roles: entity.roles.into_iter().map(|r| r.to_string()).collect(),
        })
        .collect();
    Ok(Json(ListUserConfigsByIdsResponse {
        user_configs: configs,
    }))
}

#[axum::debug_handler]
async fn list_user_privileges(
    headers: HeaderMap,
    Query(query): Query<ListUserPrivilegeRequest>,
) -> Result<Json<ListUserPrivilegeResponse>, (StatusCode, String)> {
    let platform = query.platform.map(|p| Platform::from_str(&p));
    let user_privilege_app = factory::get_user_privilege_app().await;
    let privileges = user_privilege_app
        .list_user_privilege(headers, platform)
        .await
        .log_if_error(Level::ERROR)
        .report_sentry()
        .map_err(to_http_error)?
        .into_iter()
        .map(|p| {
            let dto = p
                .backend_apis
                .into_iter()
                .map(|api| UserPrivilegeDto {
                    service: api.service,
                    method: api.method.to_string(),
                    url_path: api.url_path,
                    key: api.key,
                    associated_key: p.config_key.clone(),
                })
                .collect::<Vec<_>>();
            dto
        })
        .flatten()
        .collect::<Vec<_>>();
    Ok(Json(ListUserPrivilegeResponse {
        user_privileges: privileges,
    }))
}
