use futures::FutureExt;
use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::{sync::OnceCell as AsyncOnceLock, task::spawn_blocking};

use crate::{
    domain::repository::{
        authenticate_repository::AuthenticateRepository, cluster_repository::ClusterRepository,
        user_repository::UserRepository,
    },
    infrastructure::{
        adapter::user_identity_verify_adapter::UserIdentityVerifyHttpClient,
        repository::{
            authenticate_repo_impl::{AuthSettings, AuthenticateRepositoryImpl},
            cache::{router_cache::RoutersCache, service_instance_cache::ServiceInstanceCache},
            cluster_repo_impl::ClusterRepositoryImpl,
            user_repo_impl::UserRepositoryImpl,
        },
        utility::bb8_connection_pool::{PgPool, Settings as PgSettings},
    },
    settings::{Settings, service_conf::ServiceConf},
};

pub async fn preloading() -> anyhow::Result<()> {
    let settings = Settings::get();
    let ServiceConf {
        service_privileges,
        service_backends,
    } = spawn_blocking(move || settings.get_static_privilege_conf()).await?;

    let service_privileges = service_privileges.into_values().collect();
    let cluster_repo = get_cluster_repo().await;
    cluster_repo
        .preloading(service_privileges, service_backends)
        .await?;
    let user_repo = get_user_repo().await;
    user_repo.preloading().await?;
    Ok(())
}

/// Postgis Database connection pool.
pub(crate) async fn get_pg_pool() -> Arc<PgPool> {
    static SINGLETON: AsyncOnceLock<Arc<PgPool>> = AsyncOnceLock::const_new();
    let pool = SINGLETON
        .get_or_init(move || {
            async move {
                let settings = Settings::get();
                let pool = PgPool::new(PgSettings {
                    database_url: settings.db_args.database_url.clone(),
                    ..Default::default()
                })
                .await
                .expect("Failed to init postgis connection pool");
                Arc::new(pool)
            }
            .boxed()
        })
        .await
        .clone();
    pool
}

pub(crate) fn get_http_client() -> reqwest::Client {
    static SINGLETON: OnceLock<reqwest::Client> = OnceLock::new();
    SINGLETON
        .get_or_init(|| {
            let client = reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(2))
                .read_timeout(Duration::from_secs(5))
                .build()
                .expect("Failed to create http client");
            client
        })
        .clone()
}

pub(crate) fn get_services_instance_cache() -> ServiceInstanceCache {
    static SINGLETON: OnceLock<ServiceInstanceCache> = OnceLock::new();
    SINGLETON
        .get_or_init(|| {
            let cache = ServiceInstanceCache::new(500);
            cache
        })
        .clone()
}

pub(crate) fn get_router() -> RoutersCache {
    static SINGLETON: OnceLock<RoutersCache> = OnceLock::new();
    SINGLETON
        .get_or_init(|| {
            let cache = RoutersCache::new(None);
            cache
        })
        .clone()
}

pub(crate) async fn get_cluster_repo() -> Arc<dyn ClusterRepository> {
    static SINGLETON: AsyncOnceLock<Arc<dyn ClusterRepository>> = AsyncOnceLock::const_new();
    SINGLETON
        .get_or_init(move || {
            async move {
                let pool = get_pg_pool().await;
                let service_instance_cache = get_services_instance_cache();
                let router_cache = get_router();
                let repo = ClusterRepositoryImpl::new(pool, service_instance_cache, router_cache);
                let repo: Arc<dyn ClusterRepository> = Arc::new(repo);
                repo
            }
            .boxed()
        })
        .await
        .clone()
}

pub(crate) async fn get_user_repo() -> Arc<dyn UserRepository> {
    static SINGLETON: AsyncOnceLock<Arc<dyn UserRepository>> = AsyncOnceLock::const_new();
    SINGLETON
        .get_or_init(move || {
            async move {
                let settings = Settings::get();
                let repo: Arc<dyn UserRepository> = Arc::new(
                    UserRepositoryImpl::new(
                        get_pg_pool().await,
                        settings.initial_gateway_admin_ids.clone(),
                    )
                    .await,
                );
                repo
            }
            .boxed()
        })
        .await
        .clone()
}

pub(crate) async fn get_auth_repo() -> Arc<dyn AuthenticateRepository> {
    static SINGLETON: AsyncOnceLock<Arc<dyn AuthenticateRepository>> = AsyncOnceLock::const_new();
    SINGLETON
        .get_or_init(move || {
            async move {
                let settings = Settings::get();
                let user_identity_verify_url = settings
                    .auth_args
                    .get_user_identity_verify_url()
                    .expect("Invalid user_identity_verify_url");
                let mut auth_settings = None;
                if let Some((method, url)) = user_identity_verify_url {
                    let http_client = UserIdentityVerifyHttpClient::new(method, get_http_client());
                    auth_settings = Some(AuthSettings { url, http_client });
                }
                let service_instance_cache = get_services_instance_cache();
                let repo: Arc<dyn AuthenticateRepository> = Arc::new(
                    AuthenticateRepositoryImpl::new(auth_settings, service_instance_cache),
                );
                repo
            }
            .boxed()
        })
        .await
        .clone()
}
