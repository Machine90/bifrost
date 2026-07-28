use std::collections::HashMap;
use std::{sync::Arc, time::Duration};

use pingora::{
    lb::LoadBalancer,
    prelude::{RoundRobin, TcpHealthCheck, background_service},
    server::Server,
};

use crate::domain::model::value::service::ServiceRegistry;
#[cfg(feature = "nacos")]
use crate::settings::NacosArgs;
use crate::settings::service_conf::ServiceConf;
use crate::settings::{Args, Settings};

#[cfg(feature = "nacos")]
pub mod nacos_service_discovery;

pub fn setup(server: &mut Server) -> HashMap<ServiceRegistry, Arc<LoadBalancer<RoundRobin>>> {
    let mut upstreams = HashMap::new();
    let settings = Settings::get();
    #[cfg(feature = "nacos")]
    {
        if let Some(nacos_args) = settings.nacos_args.clone() {
            use crate::domain::model::value::service::ServiceRegistry;
            let upstream = setup_nacos_upstreams(nacos_args, server);
            upstreams.insert(ServiceRegistry::Nacos, upstream);
        }
    }
    let static_upstream =
        setup_static_upstreams(settings, server).expect("Failed to load static backend endpoints");
    if let Some(upstream) = static_upstream {
        upstreams.insert(ServiceRegistry::Static, upstream);
    }
    upstreams
}

#[cfg(feature = "nacos")]
fn setup_nacos_upstreams(
    nacos_args: NacosArgs,
    server: &mut Server,
) -> Arc<LoadBalancer<RoundRobin>> {
    use nacos_service_discovery::NacosServiceDiscovery as ND;
    use nacos_service_discovery::Settings as NDSettings;
    use pingora::lb::Backends;
    use pingora::lb::discovery::ServiceDiscovery;

    let discovery: Box<dyn ServiceDiscovery + Send + Sync + 'static> = Box::new(
        ND::new(NDSettings {
            nacos_server_address: nacos_args.nacos_server_address.clone(),
            nacos_username: nacos_args.nacos_username.clone(),
            nacos_password: nacos_args.nacos_password.into(),
            nacos_namespace: format!(""),
            services_lease_timeout_duration: Duration::from_secs(
                nacos_args.cache_service_names_secs as u64,
            ),
        })
        .expect("Failed to create nacos discovery"),
    );
    let backends = Backends::new(discovery);
    let mut nacos_upstream: LoadBalancer<RoundRobin> = LoadBalancer::from_backends(backends);
    let hc = TcpHealthCheck::new();
    nacos_upstream.set_health_check(hc);
    nacos_upstream.update_frequency = Some(Duration::from_secs(5));
    nacos_upstream.health_check_frequency = Some(Duration::from_secs(10));
    let service_discovery = background_service("nacos-discovery", nacos_upstream);
    let nacos_upstream = service_discovery.task();
    server.add_service(service_discovery);
    nacos_upstream
}

fn setup_static_upstreams(
    settings: Arc<Args>,
    server: &mut Server,
) -> anyhow::Result<Option<Arc<LoadBalancer<RoundRobin>>>> {
    let ServiceConf {
        service_backends, ..
    } = settings.get_static_privilege_conf();
    if service_backends.is_empty() {
        return Ok(None);
    }
    let backends = service_backends.into_values().flatten().collect::<Vec<_>>();
    let mut static_upstream = LoadBalancer::try_from_iter(backends.as_slice())?;
    let hc = TcpHealthCheck::new();
    static_upstream.set_health_check(hc);
    static_upstream.update_frequency = Some(Duration::from_secs(5));
    static_upstream.health_check_frequency = Some(Duration::from_secs(10));
    let service_discovery = background_service("static-discovery", static_upstream);
    let static_upstream = service_discovery.task();
    server.add_service(service_discovery);
    Ok(Some(static_upstream))
}
