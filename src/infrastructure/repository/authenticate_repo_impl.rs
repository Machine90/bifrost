use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result};
use async_trait::async_trait;
use http::HeaderMap;
use partialdebug::placeholder::PartialDebug;
use url::Url;

use crate::{
    domain::{
        model::entity::user_info::UserBaseInfo,
        repository::authenticate_repository::AuthenticateRepository,
    },
    infrastructure::{
        adapter::user_identity_verify_adapter::UserIdentityVerifyHttpClient,
        repository::cache::{
            service_instance_cache::ServiceInstanceCache, user_lease_cache::UserLeaseCache,
        },
        utility::circuit_breaker::CircuitBreaker,
    },
};

#[derive(Debug, Clone)]
pub(crate) struct AuthSettings {
    pub(crate) url: Url,
    pub(crate) http_client: UserIdentityVerifyHttpClient,
}

#[derive(PartialDebug, Clone)]
pub(crate) struct AuthenticateRepositoryImpl {
    user_lease_cache: UserLeaseCache,
    auth_settings: Option<Arc<AuthSettings>>,
    service_instance_cache: ServiceInstanceCache,
    circuit_breaker: Arc<CircuitBreaker<Url>>,
}

#[async_trait]
impl AuthenticateRepository for AuthenticateRepositoryImpl {
    async fn verify_user_identity(
        &self,
        user_info: &UserBaseInfo,
        forwarded_headers: &HeaderMap,
    ) -> Result<()> {
        if self.user_lease_cache.contains(user_info) {
            return Ok(());
        }
        if let Some(AuthSettings { url, http_client }) =
            self.auth_settings.as_ref().map(|v| v.as_ref())
        {
            let url_schema = url.scheme().to_lowercase();
            let urls = match url_schema.as_str() {
                "svc" | "service" => {
                    let service_name = url.domain().context("Service name is missing")?;
                    let mut addresses = self.find_service_backend_address(service_name).await;
                    addresses
                        .iter_mut()
                        .map(|address| {
                            address.set_path(url.path());
                            address.set_query(url.query());
                        })
                        .for_each(|_| {});
                    addresses
                }
                "http" | "https" => vec![url.clone()],
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unsupported scheme of user identity verify url: {url_schema}"
                    ));
                }
            };

            for url in urls {
                if !self.circuit_breaker.is_available(&url).await {
                    continue;
                }
                let response = http_client
                    .verify_user_identity(url.clone(), forwarded_headers)
                    .await;
                if response.is_err() {
                    self.circuit_breaker.report_failure(url).await;
                    continue;
                }
                self.circuit_breaker.report_success(url).await;
                let is_valid = response?;
                if !is_valid {
                    return Err(anyhow::anyhow!("Failed to verify user identity"));
                }
                self.user_lease_cache.add(user_info).await;
                break;
            }
        }
        Ok(())
    }
}

impl AuthenticateRepositoryImpl {
    pub(crate) fn new(
        auth_settings: Option<AuthSettings>,
        service_instance_cache: ServiceInstanceCache,
    ) -> Self {
        let user_lease_cache = UserLeaseCache::new(None, Duration::from_secs(15 * 60));
        let circuit_breaker = CircuitBreaker::new(Duration::from_secs(300), 2);
        Self {
            user_lease_cache,
            auth_settings: auth_settings.map(Arc::new),
            service_instance_cache,
            circuit_breaker: Arc::new(circuit_breaker),
        }
    }

    pub(crate) async fn find_service_backend_address(&self, service_name: &str) -> Vec<Url> {
        let service_backends = self
            .service_instance_cache
            .match_service_backends(service_name)
            .await
            .into_iter()
            .filter(|(_, backends)| backends.backend_count > 0);
        let mut urls = vec![];
        for (_, backends) in service_backends {
            let mut iter = backends.list_backends();
            while let Some(backend) = iter.get_next() {
                if let Some(socket) = backend.as_inet() {
                    let ip = socket.ip();
                    let port = socket.port();
                    let url_str = format!("http://{ip}:{port}");
                    if let Ok(address) = Url::parse(&url_str) {
                        urls.push(address);
                    }
                }
            }
        }
        urls
    }
}
