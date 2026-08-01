use std::{collections::HashMap, sync::Arc};

use pingora::{
    lb::LoadBalancer, listeners::tls::TlsSettings, prelude::RoundRobin, proxy::http_proxy_service,
    server::Server,
};

use crate::{
    domain::model::value::service::ServiceRegistry,
    pingora::components::proxy::http_proxy::{HttpProxy, Settings as ProxySettings},
    settings::Settings,
};

pub mod fail_to_proxy;
pub mod http_proxy;

pub fn setup(
    server: &mut Server,
    upstreams: HashMap<ServiceRegistry, Arc<LoadBalancer<RoundRobin>>>,
    tls_settings: Option<TlsSettings>,
) {
    let settings = Settings::get();
    let forward_roles_header = settings.auth_args.forward_roles_header.clone();
    let forward_subject_header = settings.auth_args.forward_subject_header.clone();
    let http_proxy = HttpProxy::new(
        upstreams,
        ProxySettings {
            local_http_server_port: settings.gateway_management_server_port,
            sni: settings.server_name_indication.to_string(),
            allow_unconfigured_api_exposed: settings.allow_unconfigured_api_exposed,
            allow_cors: settings.cors_args.allow_cors,
            forward_roles_header,
            forward_subject_header,
            ..Default::default()
        },
    );
    let mut http_proxy = http_proxy_service(&server.configuration, http_proxy);
    for port in settings.proxy_ports.iter() {
        if *port == 443 {
            continue;
        }
        http_proxy.add_tcp(&format!("0.0.0.0:{port}"));
    }
    if let Some(tls_settings) = tls_settings {
        http_proxy.add_tls_with_settings(&format!("0.0.0.0:443"), None, tls_settings);
    }
    server.add_service(http_proxy);
}
