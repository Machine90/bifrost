use crate::{
    application::user_privilege_app::UserPrivilegeApp,
    domain::service::{
        authentication_service::AuthenticationService,
        cluster_manage_service::ClusterManageService, proxy_service::ProxyService,
        user_manage_service::UserManageService,
    },
    infrastructure::factory as infra,
};

use futures::FutureExt;
use tokio::sync::OnceCell as AsyncOnceLock;

pub async fn preloading() -> anyhow::Result<()> {
    Ok(())
}

pub async fn get_proxy_svc() -> ProxyService {
    static SINGLETON: AsyncOnceLock<ProxyService> = AsyncOnceLock::const_new();
    SINGLETON
        .get_or_init(move || {
            async move {
                let cluster_repo = infra::get_cluster_repo().await;
                ProxyService::new(cluster_repo)
            }
            .boxed()
        })
        .await
        .clone()
}

pub async fn get_cluster_manage_svc() -> ClusterManageService {
    static SINGLETON: AsyncOnceLock<ClusterManageService> = AsyncOnceLock::const_new();
    SINGLETON
        .get_or_init(move || {
            async move {
                let cluster_repo = infra::get_cluster_repo().await;
                let user_repo = infra::get_user_repo().await;
                ClusterManageService::new(cluster_repo, user_repo)
            }
            .boxed()
        })
        .await
        .clone()
}

pub async fn get_user_manage_svc() -> UserManageService {
    static SINGLETON: AsyncOnceLock<UserManageService> = AsyncOnceLock::const_new();
    SINGLETON
        .get_or_init(move || {
            async move {
                let user_repo = infra::get_user_repo().await;
                UserManageService::new(user_repo)
            }
            .boxed()
        })
        .await
        .clone()
}

pub async fn get_authentication_svc() -> AuthenticationService {
    static SINGLETON: AsyncOnceLock<AuthenticationService> = AsyncOnceLock::const_new();
    SINGLETON
        .get_or_init(move || {
            async move {
                let user_repo = infra::get_user_repo().await;
                let auth_repo = infra::get_auth_repo().await;
                AuthenticationService::new(user_repo, auth_repo).await
            }
            .boxed()
        })
        .await
        .clone()
}

pub async fn get_user_privilege_app() -> UserPrivilegeApp {
    static SINGLETON: AsyncOnceLock<UserPrivilegeApp> = AsyncOnceLock::const_new();
    SINGLETON
        .get_or_init(move || {
            async move {
                let user_manage_svc = get_user_manage_svc().await;
                let cluster_manage_svc = get_cluster_manage_svc().await;
                UserPrivilegeApp::new(user_manage_svc, cluster_manage_svc)
            }
            .boxed()
        })
        .await
        .clone()
}
