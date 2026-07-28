use std::{
    collections::{BTreeSet, HashMap, HashSet},
    hash::{DefaultHasher, Hash, Hasher},
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
    time::Duration,
};

use anyhow::Context;
use async_trait::async_trait;
use nacos_sdk::api::{
    naming::{NamingService, NamingServiceBuilder},
    props::ClientProps,
};
use pingora::{
    Result,
    lb::{Backend, discovery::ServiceDiscovery},
};
use redact::Secret;
use tokio::{sync::RwLock, time::Instant};

use crate::{
    application, common::constants::backend, domain::model::value::service::ServiceRegistry,
};

#[derive(Debug, Clone)]
pub struct Settings {
    pub nacos_server_address: String,
    pub nacos_username: String,
    pub nacos_password: Secret<String>,
    pub nacos_namespace: String,
    pub services_lease_timeout_duration: Duration,
}

#[derive(Debug)]
pub struct Services {
    last_updated: Instant,
    should_update: bool,
    names: HashSet<String>,
    lease_timeout_duration: Duration,
}

impl Services {
    fn new(lease_timeout_duration: Duration) -> Self {
        Services {
            last_updated: Instant::now()
                .checked_add(lease_timeout_duration)
                .expect("Failed to add duration"),
            should_update: true,
            names: HashSet::new(),
            lease_timeout_duration,
        }
    }

    pub fn can_update(&self) -> bool {
        if self.should_update || self.last_updated.elapsed() > self.lease_timeout_duration {
            return true;
        }
        false
    }

    pub fn update_names<I: IntoIterator<Item = String>>(&mut self, names: I) {
        self.names = names.into_iter().collect();
        self.last_updated = Instant::now();
    }
}

pub struct NacosServiceDiscovery {
    naming_service: NamingService,
    service_count: AtomicI32,
    services: Arc<RwLock<Services>>,
}

impl NacosServiceDiscovery {
    pub fn new(settings: Settings) -> anyhow::Result<Self> {
        let naming_service = NamingServiceBuilder::new(
            ClientProps::new()
                .server_addr(settings.nacos_server_address.to_string())
                .namespace(settings.nacos_namespace)
                .auth_username(settings.nacos_username.clone())
                .auth_password(settings.nacos_password.expose_secret().clone()),
        )
        .enable_auth_plugin_http()
        .build()
        .context("Failed to build nacos naming service")?;
        Ok(Self {
            naming_service,
            service_count: AtomicI32::new(8),
            services: Arc::new(RwLock::new(Services::new(
                settings.services_lease_timeout_duration,
            ))),
        })
    }

    async fn get_service_names(&self) -> HashSet<String> {
        let read_lock = self.services.read().await;
        if !read_lock.can_update() {
            return read_lock.names.clone();
        }
        drop(read_lock);

        let service_count = self.service_count.load(Ordering::Relaxed);
        let response = self
            .naming_service
            .get_service_list(1, service_count, None)
            .await;
        let (services, total) = match response {
            Ok((services, total)) => (services, total),
            Err(err) => {
                tracing::error!(error = %err, "Failed to get service list");
                return HashSet::default();
            }
        };
        let mut update_services = self.services.write().await;
        if total != 0 && update_services.names.len() == total as usize {
            // while services count has not been changed, and there are at least 1 services
            // we should keep this state in a period of time.
            update_services.should_update = false;
        } else {
            // maybe has more changes until keep same with remote.
            update_services.should_update = true;
        }
        update_services.update_names(services.clone());
        self.service_count.store(total, Ordering::SeqCst);
        return services.into_iter().collect();
    }
}

#[async_trait]
impl ServiceDiscovery for NacosServiceDiscovery {
    async fn discover(&self) -> Result<(BTreeSet<Backend>, HashMap<u64, bool>)> {
        let services = self.get_service_names().await;

        let mut health = HashMap::new();
        let mut realtime_service_backends = HashMap::with_capacity(services.len());

        for service_name in services {
            let instances = self
                .naming_service
                .get_all_instances(service_name.clone(), None, vec![], true)
                .await
                .context(format!("service = {service_name}"))
                .context("Failed to get instances");
            let instances = match instances {
                Ok(instances) => instances,
                Err(err) => {
                    tracing::error!(error = %err, "Failed to list instance");
                    continue;
                }
            };

            let mut backends = BTreeSet::new();
            for instance in instances {
                if !instance.enabled {
                    tracing::debug!("Instance {} is disable", instance.ip_and_port());
                    continue;
                }

                let weight = (instance.weight() * backend::WEIGHT_SCALE) as usize;
                let backend = match Backend::new_with_weight(&instance.ip_and_port(), weight) {
                    Ok(backend) => backend,
                    Err(_) => continue,
                };
                let is_health = instance.healthy;

                let mut hasher = DefaultHasher::new();
                backend.hash(&mut hasher);
                let backend_hash_key = hasher.finish();

                backends.insert(backend);
                health.insert(backend_hash_key, is_health);
            }
            realtime_service_backends.insert(service_name, backends);
        }

        let cluster_manage_svc = application::factory::get_cluster_manage_svc().await;
        cluster_manage_svc
            .update_runtime_service_backends(ServiceRegistry::Nacos, &realtime_service_backends)
            .await;
        let endpoints = realtime_service_backends.into_values().flatten().collect();
        Ok((endpoints, health))
    }
}
