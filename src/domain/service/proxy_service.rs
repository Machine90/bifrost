use std::{collections::HashMap, sync::Arc};

use http::{Method, Uri};
use pingora::Result;

use crate::{
    common::pingora_errors::internal_error,
    domain::{
        model::{
            entity::{route_config::RouteConfig, service_backends::ServiceBackends},
            value::{
                platform::Platform, redirect_target::RedirectTarget, service::ServiceRegistry,
            },
        },
        repository::cluster_repository::ClusterRepository,
    },
};

#[derive(Clone)]
pub struct ProxyService {
    cluster_repo: Arc<dyn ClusterRepository>,
}

impl ProxyService {
    pub fn new(cluster_repo: Arc<dyn ClusterRepository>) -> Self {
        Self { cluster_repo }
    }

    pub async fn get_online_service_backends(
        &self,
        service_name: &str,
    ) -> HashMap<ServiceRegistry, ServiceBackends> {
        self.cluster_repo.match_service_backends(service_name).await
    }

    pub async fn match_route_config(
        &self,
        platform: &Platform,
        redirect_target: &RedirectTarget,
        method: &Method,
        uri: &Uri,
    ) -> Result<Option<Arc<RouteConfig>>> {
        let service = redirect_target.get_service_name();
        let route_config = self
            .cluster_repo
            .get_route_config(platform, &service, method, uri.path(), true)
            .await
            .map_err(|_| internal_error())?;
        return Ok(route_config);
    }
}
