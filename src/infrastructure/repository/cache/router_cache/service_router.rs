use std::sync::Arc;

use anyhow::{Context, Result};
use matchit::{MatchError, Router};
use moka2::future::Cache;
use tokio::sync::Mutex;
use tracing::Level;

use crate::{
    common::{sentry_ext::SentryError, tracing_ext::TracingResultExt},
    domain::model::{
        entity::route_config::{RouteConfig, RouterPath, UrlKind},
        value::platform::Platform,
    },
};

const DEFAULT_MAX_SERVICE_ABSOLUTELY_URL_PATH_COUNT: u64 = 100_000;

#[derive(Debug, Clone)]
pub struct ServiceRouter {
    pub write_only_guard: Arc<Mutex<()>>,
    /// This router used to save path such as '/home/user', '/home/user/{{hello}}'
    absolutely_path: Arc<Cache<String, Arc<RouteConfig>>>,
    /// This router used to save path such as '/home/user/{id}'
    match_path_router: matchit::Router<Arc<RouteConfig>>,
    /// This router used to save path such as '/home/{*rest}'
    match_all_router: matchit::Router<Arc<RouteConfig>>,
}

impl ServiceRouter {
    pub fn new() -> Self {
        Self {
            write_only_guard: Arc::new(Mutex::new(())),
            absolutely_path: Arc::new(Cache::new(DEFAULT_MAX_SERVICE_ABSOLUTELY_URL_PATH_COUNT)),
            match_path_router: Default::default(),
            match_all_router: Default::default(),
        }
    }

    pub async fn match_route_config(
        &self,
        route_path_str: &str,
        platform: &Platform,
    ) -> Option<Arc<RouteConfig>> {
        let mut config = None;
        if config.is_none() {
            config = self
                .absolutely_path
                .get(route_path_str)
                .await
                .clone()
                .filter(|c| c.support_platform(platform));
        }

        if config.is_none() {
            config = match self.match_path_router.at(&route_path_str) {
                Ok(node) => Some(node.value.clone()),
                Err(MatchError::NotFound) => None,
            }
            .filter(|c| c.support_platform(platform));
        }

        if config.is_none() {
            config = match self.match_all_router.at(&route_path_str) {
                Ok(node) => Some(node.value.clone()),
                Err(MatchError::NotFound) => None,
            }
            .filter(|c| c.support_platform(platform));
        }
        config
    }

    pub async fn get_route_config(
        &self,
        platform: &Platform,
        configured_route_path: RouterPath,
    ) -> Option<Arc<RouteConfig>> {
        let route_kind = configured_route_path.url_kinds();
        let url = configured_route_path.to_string();
        let config = match route_kind {
            UrlKind::Absolutely => self.absolutely_path.get(&url).await,
            UrlKind::MatchPath => {
                let config = self.match_path_router.at(&url).ok()?.value.clone();
                Some(config)
            }
            UrlKind::MatchAll => {
                let config = self.match_all_router.at(&url).ok()?.value.clone();
                Some(config)
            }
        }
        .filter(|c| c.support_platform(platform));
        config
    }

    pub async fn upsert_router(
        &mut self,
        configured_route_path: RouterPath,
        new_config: RouteConfig,
        overwrite_config: bool,
    ) -> anyhow::Result<Arc<RouteConfig>> {
        let config = match configured_route_path.url_kinds() {
            UrlKind::Absolutely => {
                self.upsert_absolutely_router(configured_route_path, new_config, overwrite_config)
                    .await?
            }
            UrlKind::MatchPath => {
                self.upsert_match_path_router(configured_route_path, new_config, overwrite_config)
                    .await?
            }
            UrlKind::MatchAll => {
                self.upsert_match_all_router(configured_route_path, new_config, overwrite_config)
                    .await?
            }
        };
        Ok(config)
    }

    pub async fn upsert_absolutely_router(
        &self,
        configured_route_path: RouterPath,
        mut new_config: RouteConfig,
        overwrite_config: bool,
    ) -> anyhow::Result<Arc<RouteConfig>> {
        let route_path_str = configured_route_path.to_string();
        let previous_config = self.absolutely_path.get(&route_path_str).await;

        if let Some(previous_config) = previous_config {
            let mut previous = previous_config.as_ref().clone();
            previous.merge_roles(&new_config, overwrite_config);
            previous.merge_ratelimiter(&new_config);
            previous.merge_retry_policy(&new_config);
            new_config = previous;
        }

        let new_config = Arc::new(new_config);
        self.absolutely_path
            .insert(route_path_str, new_config.clone())
            .await;

        Ok(new_config)
    }

    pub async fn upsert_match_path_router(
        &mut self,
        configured_route_path: RouterPath,
        new_config: RouteConfig,
        overwrite_config: bool,
    ) -> anyhow::Result<Arc<RouteConfig>> {
        let route_path_str = configured_route_path.to_string();
        let first_tried = Arc::new(new_config.clone());

        // try to insert new configure
        let try_insert = self
            .match_path_router
            .insert(&route_path_str, first_tried.clone());

        if try_insert.is_ok() {
            return Ok(first_tried);
        }

        // otherwise there are some conflicts between origin router and new route path.
        let new_config = update_matchit_router(
            &mut self.match_path_router,
            configured_route_path,
            new_config,
            overwrite_config,
        )
        .await?;
        Ok(new_config)
    }

    pub async fn upsert_match_all_router(
        &mut self,
        configured_route_path: RouterPath,
        new_config: RouteConfig,
        overwrite_config: bool,
    ) -> anyhow::Result<Arc<RouteConfig>> {
        let route_path_str = configured_route_path.to_string();
        let first_tried = Arc::new(new_config.clone());

        // try to insert new configure
        let try_insert = self
            .match_all_router
            .insert(&route_path_str, first_tried.clone());

        if try_insert.is_ok() {
            return Ok(first_tried);
        }

        // otherwise there are some conflicts between origin router and new route path.
        let new_config = update_matchit_router(
            &mut self.match_all_router,
            configured_route_path,
            new_config,
            overwrite_config,
        )
        .await?;
        Ok(new_config)
    }

    pub async fn remove_route_config(
        &mut self,
        route: RouterPath,
        platform: &Platform,
    ) -> Result<()> {
        let route_path = route.to_string();
        match route.url_kinds() {
            UrlKind::Absolutely => {
                if let Some((new_config, router)) = self
                    .absolutely_path
                    .remove(&route_path)
                    .await
                    .zip(Some(&mut self.absolutely_path))
                    .map(|(previous_config, router)| {
                        let new_config = remove_platform_from_config(previous_config, platform)?;
                        Some((new_config, router))
                    })
                    .flatten()
                {
                    router
                        .insert(route_path.clone(), Arc::new(new_config))
                        .await;
                }
            }
            UrlKind::MatchPath => {
                if let Some((new_config, router)) = self
                    .match_path_router
                    .remove(&route_path)
                    .zip(Some(&mut self.match_path_router))
                    .map(|(previous_config, router)| {
                        let new_config = remove_platform_from_config(previous_config, platform)?;
                        Some((new_config, router))
                    })
                    .flatten()
                {
                    router.insert(route_path.clone(), Arc::new(new_config))?;
                }
            }
            UrlKind::MatchAll => {
                if let Some((new_config, router)) = self
                    .match_all_router
                    .remove(&route_path)
                    .zip(Some(&mut self.match_all_router))
                    .map(|(previous_config, router)| {
                        let new_config = remove_platform_from_config(previous_config, platform)?;
                        Some((new_config, router))
                    })
                    .flatten()
                {
                    router.insert(route_path.clone(), Arc::new(new_config))?;
                }
            }
        };
        Ok(())
    }
}

async fn update_matchit_router(
    router: &mut Router<Arc<RouteConfig>>,
    configured_route_path: RouterPath,
    mut new_config: RouteConfig,
    overwrite_config: bool,
) -> anyhow::Result<Arc<RouteConfig>> {
    let route_path_str = configured_route_path.to_string();
    let mut real_route_path = configured_route_path.clone();

    if let Ok(ori_route_path) = router.at(&route_path_str).map(|n| n.value.route_path()) {
        // the target url path was covered by other path rule.
        real_route_path = ori_route_path
    }

    // try to find previous config from router, then merge it if exists.
    let previous_config = router.remove(real_route_path.to_string());
    match previous_config {
        Some(previous) => {
            let mut previous = previous.as_ref().clone();
            previous.merge_roles(&new_config, overwrite_config);
            previous.merge_ratelimiter(&new_config);
            previous.merge_retry_policy(&new_config);
            new_config = previous;
        }
        None => (),
    };
    let new_config = Arc::new(new_config);
    router
        .insert(route_path_str, new_config.clone())
        .context(format!("url_path = {configured_route_path:?}"))
        .context("Failed to add route config")
        .report_sentry()
        .log_if_error(Level::ERROR)?;
    Ok(new_config)
}

fn remove_platform_from_config(
    previous_config: Arc<RouteConfig>,
    platform: &Platform,
) -> Option<RouteConfig> {
    let mut previous_config = previous_config.as_ref().clone();
    // try to remove platform and its roles from config
    previous_config.remove_platform(platform);
    if previous_config.is_empty() {
        None
    } else {
        Some(previous_config)
    }
}
