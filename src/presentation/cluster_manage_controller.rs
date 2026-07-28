use aide::axum::{
    ApiRouter,
    routing::{get, post},
};
use axum::{Extension, Json, extract::Query};
use http::StatusCode;
use unic_langid::LanguageIdentifier;

use crate::{
    application::factory,
    common::{constants::backend, error_to_http_status::to_http_error},
    domain::model::entity::service_backends::ServiceBackends,
    infrastructure,
    presentation::dto::cluster_manage::{
        BackendInfo, ClusterService, ListClusterPlatformResponse, ListClusterRolesRequest,
        ListClusterRolesResponse, ListClusterServiceRequest, ListClusterServiceResponse,
        ListServiceBackendRequest, ListServiceBackendResponse, PlatformEntry, RenameRoleRequest,
        RenameRoleResponse, RoleEntry,
    },
};

pub mod url {
    use constcat::concat;

    use crate::common::constants::http_server::BASE_URL;

    pub const CLUSTER_SERVICE_LIST: &str = concat!(BASE_URL, "/service/list");
    pub const CLUSTER_BACKEND_LIST: &str = concat!(BASE_URL, "/backend/list");
    pub const CLUSTER_ROLE_LIST: &str = concat!(BASE_URL, "/role/list");
    pub const CLUSTER_PLATFORM_LIST: &str = concat!(BASE_URL, "/platform/list");

    pub const CLUSTER_RENAME_ROLE: &str = concat!(BASE_URL, "/role/rename");

    pub const CLUSTER_DEBUG: &str = concat!(BASE_URL, "/debug");
}

pub(crate) fn register(router: ApiRouter) -> ApiRouter {
    router
        .api_route(url::CLUSTER_SERVICE_LIST, get(list_services))
        .api_route(url::CLUSTER_BACKEND_LIST, get(list_backend_of_service))
        .api_route(url::CLUSTER_ROLE_LIST, get(list_roles))
        .api_route(url::CLUSTER_PLATFORM_LIST, get(list_platforms))
        .api_route(url::CLUSTER_RENAME_ROLE, post(rename_role))
        .route(url::CLUSTER_DEBUG, get(debug_cluster))
}

#[axum::debug_handler]
async fn list_services(
    Query(_queries): Query<ListClusterServiceRequest>,
) -> Result<Json<ListClusterServiceResponse>, StatusCode> {
    let cluster_manage_svc = factory::get_cluster_manage_svc().await;
    let names = cluster_manage_svc.list_online_services();
    let response = ListClusterServiceResponse {
        service_names: names
            .into_iter()
            .map(|svc| {
                let name = svc.get_name().to_string();
                let registry = svc.get_registry().get_name().to_string();
                ClusterService { registry, name }
            })
            .collect(),
    };
    Ok(Json(response))
}

#[axum::debug_handler]
async fn list_backend_of_service(
    Query(queries): Query<ListServiceBackendRequest>,
) -> Result<Json<ListServiceBackendResponse>, StatusCode> {
    let cluster_manage_svc = factory::get_proxy_svc().await;
    let service_backends = cluster_manage_svc
        .get_online_service_backends(&queries.service_name)
        .await;
    let service_backends = service_backends
        .values()
        .map(ServiceBackends::list_backends)
        .collect::<Vec<_>>();
    let mut result = vec![];
    for mut backends in service_backends {
        while let Some(backend) = backends.get_next() {
            let backend_address = backend.addr.to_string();
            let weight = backend.weight as f64 / backend::WEIGHT_SCALE;
            result.push(BackendInfo {
                address: backend_address,
                weight,
            });
        }
    }
    let response = ListServiceBackendResponse { backends: result };
    Ok(Json(response))
}

#[axum::debug_handler]
async fn list_roles(
    Extension(prefer_language): Extension<LanguageIdentifier>,
    Query(_queries): Query<ListClusterRolesRequest>,
) -> Result<Json<ListClusterRolesResponse>, StatusCode> {
    let cluster_svc = factory::get_cluster_manage_svc().await;
    let roles = cluster_svc.get_distinct_roles().await;
    let roles = roles
        .iter()
        .map(|role| {
            let key = role.to_string();
            let display_name = role.display_name(&prefer_language);
            RoleEntry { key, display_name }
        })
        .collect();
    Ok(Json(ListClusterRolesResponse { roles }))
}

#[axum::debug_handler]
async fn list_platforms(
    Extension(prefer_language): Extension<LanguageIdentifier>,
) -> Result<Json<ListClusterPlatformResponse>, StatusCode> {
    let cluster_svc = factory::get_cluster_manage_svc().await;
    let platforms = cluster_svc.get_distinct_platform().await;
    let platforms = platforms
        .iter()
        .map(|p| PlatformEntry {
            key: p.as_str().to_string(),
            display_name: p.display_name(&prefer_language),
        })
        .collect();
    Ok(Json(ListClusterPlatformResponse { platforms }))
}

#[axum::debug_handler]
async fn rename_role(
    Json(request): Json<RenameRoleRequest>,
) -> Result<Json<RenameRoleResponse>, (StatusCode, String)> {
    let RenameRoleRequest { old_role, new_role } = request;
    let cluster_svc = factory::get_cluster_manage_svc().await;
    let renamed = cluster_svc
        .rename_role(&old_role, &new_role)
        .await
        .map_err(to_http_error)?;
    Ok(Json(RenameRoleResponse {
        updated_privilege_config_count: renamed.updated_privilege_count,
        updated_user_config_count: renamed.updated_user_count,
    }))
}

#[axum::debug_handler]
async fn debug_cluster() -> Result<Json<String>, StatusCode> {
    let router = infrastructure::factory::get_router();
    let debug_router = format!("{router:#?}");
    Ok(Json(debug_router))
}
