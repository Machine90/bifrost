use std::sync::Arc;

use http::Method;
use moka2::future::Cache;

use crate::{
    domain::model::{
        entity::route_config::{RouteConfig, RouterPath},
        value::platform::Platform,
    },
    infrastructure::repository::cache::router_cache::service_router::ServiceRouter,
};

pub(crate) mod service_router;

/// Default to 100,000, if the service number larger than this value,
/// what a large cluster it is!
const DEFAULT_MAX_SERVICE_COUNT: u64 = 100_000;

#[derive(Debug, Clone)]
pub struct RoutersCache {
    services: Arc<Cache<String, Arc<ServiceRouter>>>,
}

impl RoutersCache {
    pub(crate) fn new(max_svc_count: Option<u64>) -> Self {
        let cap = max_svc_count.unwrap_or(DEFAULT_MAX_SERVICE_COUNT);
        Self {
            services: Arc::new(Cache::new(cap)),
        }
    }

    async fn match_route_config_internal(
        &self,
        platform: &Platform,
        service_name: &str,
        method: &Method,
        url_path: &str,
    ) -> Option<Arc<RouteConfig>> {
        let service_router = match self.services.get(service_name).await {
            Some(service) => service,
            _ => return None,
        };
        let route_path_str = RouterPath {
            method: method.clone(),
            url_path: url_path.to_string(),
        }
        .to_string();
        let config = service_router
            .match_route_config(&route_path_str, platform)
            .await;
        config
    }

    pub(crate) async fn match_route_config(
        &self,
        platform: &Platform,
        service_name: &str,
        method: &Method,
        url_path: &str,
    ) -> Option<Arc<RouteConfig>> {
        let config = self
            .match_route_config_internal(platform, service_name, method, url_path)
            .await?;
        Some(config)
    }

    /// Get the route node in difference layer by router path, unlike the `match_route_config`,
    /// this method will not to match the url path in others layer.
    pub(crate) async fn get_route_exactly(
        &self,
        platform: &Platform,
        service_name: &str,
        configured_route_path: RouterPath,
    ) -> Option<Arc<RouteConfig>> {
        let service_router = self.services.get(service_name).await?;
        let config = service_router
            .get_route_config(platform, configured_route_path)
            .await;
        config
    }

    /// Upsert route config of specified service path, then return new config reference.
    ///
    /// ## Params
    /// **service_name**: the route rule belongs to which service.
    /// **new_config**: config of route to be updated.
    /// **overwrite_url_path**: overwrite the path if `true`, for
    /// example the new path of `new_config` is `/home/{id}`, but
    /// there're already exists route path `/home/{name}`, then previous
    /// path will be replaced.
    pub(crate) async fn upsert_route_config(
        &self,
        service_name: &str,
        new_config: RouteConfig,
        overwrite_config: bool,
    ) -> anyhow::Result<Arc<RouteConfig>> {
        let configured_route_path = new_config.route_path();

        // clone a temporary router for updating.
        let mut service_router = self
            .services
            .entry(service_name.to_string())
            .or_insert(Arc::new(ServiceRouter::new()))
            .await
            .value()
            .as_ref()
            .clone();

        // make sure each time allow exactly 1 process to update router.
        let mutex = service_router.write_only_guard.clone();
        let _guard = mutex.lock().await;

        let config = service_router
            .upsert_router(configured_route_path, new_config, overwrite_config)
            .await?;

        // add modified service to services map
        self.services
            .insert(service_name.to_string(), Arc::new(service_router))
            .await;
        Ok(config)
    }

    pub(crate) async fn prune_platform_route_config(
        &self,
        platform: &Platform,
        service_name: &str,
        method: &Method,
        url_path: &str,
    ) -> anyhow::Result<()> {
        // get a cloned temporary service router config, all operation will be happened on
        // this copy.
        let mut service_router = match self.services.get(service_name).await {
            Some(service) => service.as_ref().clone(),
            _ => return Ok(()),
        };

        let route = RouterPath {
            method: method.clone(),
            url_path: url_path.to_string(),
        };

        let mutex = service_router.write_only_guard.clone();
        let _guard = mutex.lock().await;

        service_router.remove_route_config(route, platform).await?;

        // add modified service to services map
        self.services
            .insert(service_name.to_string(), Arc::new(service_router))
            .await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use http::Method;
    use rstest::rstest;

    use crate::{
        domain::model::{
            entity::route_config::{RouteConfig, RouterPath},
            value::{platform::Platform, role::Role},
        },
        infrastructure::repository::cache::router_cache::RoutersCache,
    };

    const SERVICE_NAME: &str = "test";

    #[rstest]
    #[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 1))]
    async fn test_router_match() -> anyhow::Result<()> {
        let router = RoutersCache::new(10.into());
        router
            .upsert_route_config(
                SERVICE_NAME,
                RouteConfig::new_without_config(
                    &Method::GET,
                    "/home/{*rest}",
                    [
                        (
                            Platform::Gateway,
                            [Role::Tagged("A".to_string()), Role::Anonymous].into(),
                        ),
                        (
                            Platform::Platform("admin".to_string()),
                            [Role::Tagged("B".to_string()), Role::GatewayAdmin].into(),
                        ),
                    ],
                ),
                true,
            )
            .await
            .expect("Failed to insert config");
        let conf = router
            .match_route_config(&Platform::Gateway, SERVICE_NAME, &Method::GET, "/home")
            .await;
        assert!(conf.is_none());

        let conf = router
            .match_route_config(&Platform::Gateway, SERVICE_NAME, &Method::GET, "/home/city")
            .await;
        assert!(conf.is_some());

        let conf = router
            .match_route_config(
                &Platform::Gateway,
                SERVICE_NAME,
                &Method::GET,
                "/home/city/town",
            )
            .await;
        assert!(conf.is_some());
        let conf = conf.unwrap();
        assert_eq!(conf.route_path().to_string(), "GET:/home/{*rest}");

        // "/home/{id}" not allowed to replace "/home/{*rest}" when
        // `overwrite_url_path` set to false.
        let result = router
            .upsert_route_config(
                SERVICE_NAME,
                RouteConfig::new_without_config(
                    &Method::GET,
                    "/home/{id}",
                    [(
                        Platform::Gateway,
                        [Role::Tagged("C".to_string()), Role::Untagged].into(),
                    )],
                ),
                true,
            )
            .await?;
        assert_eq!(result.route_path().to_string(), "GET:/home/{id}");
        assert!(
            result.is_allowed_roles(&Platform::Gateway, &[Role::Tagged("C".to_string())].into())
        );
        // B has been override
        assert!(
            !result.is_allowed_roles(&Platform::Gateway, &[Role::Tagged("B".to_string())].into())
        );

        // "/home/{id}" will replace "/home/{*rest}"
        let result = router
            .upsert_route_config(
                SERVICE_NAME,
                RouteConfig::new_without_config(
                    &Method::GET,
                    "/home/{id}",
                    [(
                        Platform::Gateway,
                        [Role::Tagged("A".to_string()), Role::Untagged].into(),
                    )],
                ),
                false,
            )
            .await
            .expect("Failed to insert config");
        // url path has been override
        assert_eq!(result.route_path().to_string(), "GET:/home/{id}");
        assert!(
            result.is_allowed_roles(&Platform::Gateway, &[Role::Tagged("A".to_string())].into())
        );
        assert!(
            result.is_allowed_roles(&Platform::Gateway, &[Role::Tagged("C".to_string())].into())
        );

        let conf = router
            .match_route_config(&Platform::Gateway, SERVICE_NAME, &Method::GET, "/home/123")
            .await
            .expect("Rule config is missing");
        assert_ne!(conf.route_path().to_string(), "/home/{*rest}");

        router
            .upsert_route_config(
                SERVICE_NAME,
                RouteConfig::new_without_config(
                    &Method::GET,
                    "/home",
                    [(Platform::Gateway, [Role::Tagged("C".to_string())].into())],
                ),
                true,
            )
            .await
            .expect("Failed to insert config");
        let conf = router
            .match_route_config(&Platform::Gateway, SERVICE_NAME, &Method::GET, "/home")
            .await;
        assert!(conf.is_some());
        let conf = conf.unwrap();

        let dev_platform = Platform::Platform("developer".to_string());
        assert!(!conf.support_platform(&dev_platform));

        router
            .upsert_route_config(
                SERVICE_NAME,
                RouteConfig::new_without_config(
                    &Method::GET,
                    "/home",
                    [(dev_platform.clone(), [Role::Tagged("C".to_string())].into())],
                ),
                true,
            )
            .await
            .expect("Failed to insert config");
        let conf = router
            .match_route_config(&Platform::Gateway, SERVICE_NAME, &Method::GET, "/home")
            .await
            .expect("Rule config is missing");
        assert!(conf.support_platform(&dev_platform));

        assert!(!conf.is_allowed_roles(&dev_platform, &[Role::Tagged("D".to_string())].into()));
        router
            .upsert_route_config(
                SERVICE_NAME,
                RouteConfig::new_without_config(
                    &Method::GET,
                    "/home",
                    [(
                        Platform::Platform("developer".to_string()),
                        [Role::Tagged("C".to_string()), Role::Tagged("D".to_string())].into(),
                    )],
                ),
                false,
            )
            .await
            .expect("Rule config is missing");
        let conf = router
            .match_route_config(&Platform::Gateway, SERVICE_NAME, &Method::GET, "/home")
            .await
            .expect("Rule config is missing");
        assert!(conf.is_allowed_roles(&dev_platform, &[Role::Tagged("D".to_string())].into()));

        Ok(())
    }

    #[rstest]
    #[case("/home/{id}", "/home/1")]
    #[case("/home/user", "/home/user2")]
    #[case("/home/{{user}}", "/home/user")]
    #[case("/home/{*rest}", "/home/user2")]
    #[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 1))]
    async fn test_prune_route_config(#[case] url_path: &str, #[case] try_match_url_path: &str) {
        let router = RoutersCache::new(10.into());
        let _ = router
            .upsert_route_config(
                SERVICE_NAME,
                RouteConfig::new_without_config(
                    &Method::GET,
                    url_path,
                    [
                        (Platform::Gateway, [Role::Tagged("A".to_string())].into()),
                        (
                            Platform::Platform("Payment".to_string()),
                            [Role::Tagged("B".to_string())].into(),
                        ),
                    ],
                ),
                true,
            )
            .await
            .expect("Failed to insert new config");

        // get router exactly without match url pattern, this must return some config.
        let config = router
            .get_route_exactly(
                &Platform::Gateway,
                SERVICE_NAME,
                RouterPath {
                    method: Method::GET,
                    url_path: url_path.to_string(),
                },
            )
            .await;
        assert!(config.is_some());

        // this case will get nothing.
        let config = router
            .get_route_exactly(
                &Platform::Gateway,
                SERVICE_NAME,
                RouterPath {
                    method: Method::GET,
                    url_path: try_match_url_path.to_string(),
                },
            )
            .await;
        assert!(config.is_none());

        // try to remove the config from route node.
        router
            .prune_platform_route_config(&Platform::Gateway, SERVICE_NAME, &Method::GET, url_path)
            .await
            .expect("Failed to remove config");

        // the config of given path on Gateway platform should be removed
        let config = router
            .get_route_exactly(
                &Platform::Gateway,
                SERVICE_NAME,
                RouterPath {
                    method: Method::GET,
                    url_path: url_path.to_string(),
                },
            )
            .await;
        assert!(config.is_none());

        // but on Payment platform should be kept.
        let config = router
            .get_route_exactly(
                &Platform::Platform("Payment".to_string()),
                SERVICE_NAME,
                RouterPath {
                    method: Method::GET,
                    url_path: url_path.to_string(),
                },
            )
            .await;
        assert!(config.is_some());
    }
}
