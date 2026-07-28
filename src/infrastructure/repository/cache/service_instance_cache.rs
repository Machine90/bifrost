use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fmt::Debug,
    sync::Arc,
};

use moka2::future::Cache;
use pingora::lb::Backend;

use crate::domain::model::{
    entity::service_backends::ServiceBackends,
    value::service::{Service, ServiceRegistry},
};

impl Debug for ServiceInstanceCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrivilegeCache")
            .field(
                "cache_weighted_size",
                &self.service_instances.weighted_size(),
            )
            .finish()
    }
}

#[derive(Clone)]
pub(crate) struct ServiceInstanceCache {
    service_instances: Arc<Cache<ServiceRegistry, Arc<HashMap<Service, ServiceBackends>>>>,
}

impl ServiceInstanceCache {
    pub(crate) fn new(max_registry_count: u64) -> Self {
        Self {
            service_instances: Arc::new(Cache::new(max_registry_count)),
        }
    }

    pub(crate) fn list_services(&self) -> HashSet<Service> {
        self.service_instances
            .iter()
            .map(|(_, registry)| {
                registry
                    .iter()
                    .map(|(svc, _)| svc.clone())
                    .collect::<Vec<_>>()
            })
            .flatten()
            .collect()
    }

    pub(crate) async fn get_service_backends(&self, service: Service) -> Option<ServiceBackends> {
        let registry = service.get_registry();
        let backends = self
            .service_instances
            .get(&registry)
            .await?
            .get(&registry.into_service(service.get_name()))
            .map(Clone::clone);
        backends
    }

    pub(crate) async fn match_service_backends(
        &self,
        service_name: &str,
    ) -> HashMap<ServiceRegistry, ServiceBackends> {
        let mut services = vec![];
        services.push(Service::Static(service_name.to_string()));
        #[cfg(feature = "nacos")]
        {
            services.push(Service::Nacos(service_name.to_string()));
        }
        let mut result = HashMap::new();
        for service in services {
            let registry = service.get_registry();
            if let Some(backends) = self.get_service_backends(service).await {
                result.insert(registry, backends);
            }
        }
        result
    }

    pub(crate) async fn set_service_backends(
        &self,
        registry: ServiceRegistry,
        service_instances: &HashMap<String, BTreeSet<Backend>>,
    ) {
        let service_backends = service_instances.iter().fold(
            HashMap::new(),
            |mut service_backends, (service, backends)| {
                let backends = ServiceBackends::new(service, backends);
                service_backends.insert(registry.into_service(service), backends);
                service_backends
            },
        );
        self.service_instances
            .insert(registry, Arc::new(service_backends))
            .await;
    }
}
