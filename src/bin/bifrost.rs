use bifrost::{
    common::tracing_guard::{self, LogFile, setup_tracing},
    pingora::{
        components::{proxy, service_discovery, tls::get_tls_settings},
        services::{self},
    },
    settings::Settings,
};
use pingora::prelude::*;
use tracing::level_filters::LevelFilter;

fn main() {
    let settings = Settings::get();
    settings.check();
    let pingora_options = Opt {
        conf: settings
            .pingora_conf_path
            .clone()
            .map(|p| p.to_str().map(ToString::to_string))
            .flatten(),
        ..Default::default()
    };

    let _tracing_guard = setup_tracing(tracing_guard::Settings {
        project_name: "Bifrost".to_string(),
        environment: settings.env.to_string(),
        sentry_dsn: settings.sentry_args.dsn.clone(),
        log_file: Some(LogFile {
            level_filter: LevelFilter::INFO,
            strategy: tracing_guard::RollingStrategy::Daily,
            path: "./logs".into(),
        }),
        ..Default::default()
    })
    .expect("Failed to setup tracing");

    let server = Server::new(pingora_options);
    let mut server = match server {
        Ok(server) => server,
        Err(_) => {
            panic!("Failed to create server");
        }
    };
    server.bootstrap();
    let upstreams = service_discovery::setup(&mut server);
    proxy::setup(&mut server, upstreams, get_tls_settings());
    services::setup(&mut server);
    server.run_forever();
}
