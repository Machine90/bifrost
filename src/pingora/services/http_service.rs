use aide::{
    axum::ApiRouter,
    openapi::{Info, OpenApi},
};
use anyhow::Result;
use async_trait::async_trait;
use axum::{Json, Router, extract::DefaultBodyLimit, routing::get};
use axum_l10n::LanguageIdentifierExtractorLayer;
#[cfg(unix)]
use pingora::server::ListenFds;
use pingora::{server::ShutdownWatch, services::Service};

use crate::{
    application,
    common::{
        constants::http_server::OPENAPI_URL,
        iso639_identifier::iso639_identifier::{POPULAR_ISO639_IDS, ZH},
    },
    infrastructure,
    presentation::{
        cluster_manage_controller, privilege_manage_controller, user_manage_controller,
    },
    settings::Settings,
};

const MAX_LIMIT_REQUEST_BODY_BYTES: usize = 128 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct HttpService {}

impl HttpService {
    pub fn new() -> Self {
        Self {}
    }
}

pub fn routes() -> Router {
    let mut openapi = OpenApi {
        info: Info {
            title: format!("Bifrost"),
            description: Some("Bifrost gateway API doc".to_string()),
            ..Info::default()
        },
        ..OpenApi::default()
    };
    let router = ApiRouter::new();
    let router = cluster_manage_controller::register(router);
    let router = privilege_manage_controller::register(router);
    let router = user_manage_controller::register(router);

    let router = router.nest_service("/static", tower_http::services::ServeDir::new("./static"));
    router
        .layer(DefaultBodyLimit::max(MAX_LIMIT_REQUEST_BODY_BYTES))
        .layer(LanguageIdentifierExtractorLayer::new(
            ZH,
            POPULAR_ISO639_IDS.to_vec(),
            axum_l10n::RedirectMode::NoRedirect,
        ))
        .finish_api(&mut openapi)
        .route(OPENAPI_URL, get(|| async { Json(openapi) }))
}

pub async fn run_http_server(router: Router, _shutdown_signal: ShutdownWatch) -> Result<()> {
    let port = Settings::get().gateway_management_server_port;
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    // Start the server
    axum::serve(listener, router.into_make_service()).await?;
    Ok(())
}

async fn preloading() -> Result<()> {
    infrastructure::factory::preloading().await?;
    application::factory::preloading().await?;
    Ok(())
}

#[async_trait]
impl Service for HttpService {
    async fn start_service(
        &mut self,
        #[cfg(unix)] _: Option<ListenFds>,
        shutdown: ShutdownWatch,
        _listeners_per_fd: usize,
    ) {
        preloading()
            .await
            .expect("Failed to preloading resources of gateway");
        // TODO: handle shutdown
        run_http_server(routes(), shutdown)
            .await
            .expect("Failed to boot http server");
    }

    fn name(&self) -> &str {
        "bifrost-http-service"
    }
}
