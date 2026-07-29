use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, anyhow};
use async_trait::async_trait;
use http::{HeaderMap, Method, StatusCode, header};
use pingora::{
    Error, Result,
    http::{IntoCaseHeaderName as _, ResponseHeader},
    lb::{Backend, LoadBalancer},
    prelude::{HttpPeer, RoundRobin},
    protocols::l4::socket::SocketAddr,
    proxy::{ProxyHttp, Session},
};
use real_ip::real_ip;
use tracing::Level;

use crate::{
    application::factory,
    common::{
        constants::{customized_headers, http_proxy, http_server},
        header_map_ext::GetIgnoreCase,
        pingora_errors::{forbidden, internal_error, notfound},
        tracing_ext::TracingResultExt,
    },
    domain::model::{
        entity::{route_config::RouteConfig, user_info::UserBaseInfo},
        value::{
            platform::Platform, redirect_target::RedirectTarget, role::Role,
            service::ServiceRegistry, service_extracted_from::ServiceExtractFrom,
        },
    },
};

const LOCALHOST_IP: &str = "0.0.0.0";
/// Default to 24 hours
const ACCESS_CONTROL_MAX_AGE_HOURS: &str = "86400";

#[derive(Debug)]
enum RealIp {
    /// from domain socket
    DomainSocket,
    Remote(IpAddr),
}

impl RealIp {
    pub fn get_client_id(&self, redirect: &RedirectTarget) -> String {
        let host = match self {
            RealIp::DomainSocket => LOCALHOST_IP.to_string(),
            RealIp::Remote(ip_addr) => ip_addr.to_string(),
        };
        let service_name = match redirect {
            RedirectTarget::Service { service_name, .. } => service_name,
            RedirectTarget::Localhost => "gateway",
        };
        format!("{service_name}:{host}")
    }
}

#[derive(Default, Debug)]
pub struct ProxyContext {
    redirect_target: RedirectTarget,
    request_from: Platform,
    real_ip: Option<RealIp>,
    to_service: Option<String>,
    to_backend: Option<Backend>,
    route_config: Option<Arc<RouteConfig>>,
    tries: u32,
    user_info: Option<UserBaseInfo>,
    /// true if the user identity after checking from authentication service.
    is_valid_user: Option<bool>,
    /// Log request context as `info` in production env
    log_request_as_info: bool,
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub local_http_server_port: u16,
    pub sni: String,
    pub allow_unconfigured_api_exposed: bool,
    pub allowed_connection_timeout: Option<Duration>,
    pub allowed_read_timeout: Option<Duration>,
    pub allowed_write_timeout: Option<Duration>,

    pub allow_cors: bool,
    pub cors_allowed_origins: HashSet<String>,
    pub cors_allowed_methods: Vec<String>,
    pub cors_allowed_headers: Vec<String>,

    pub forward_roles_header: Option<String>,
    pub forward_subject_header: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            local_http_server_port: http_server::DEFAULT_HTTP_PORT,
            sni: http_proxy::DEFAULT_SNI.to_string(),
            allow_unconfigured_api_exposed: false,
            allowed_connection_timeout: Some(Duration::from_secs(5)),
            allowed_read_timeout: Some(Duration::from_secs(30)),
            allowed_write_timeout: Some(Duration::from_secs(60)),
            allow_cors: true,
            cors_allowed_origins: [format!("*")].into(),
            cors_allowed_methods: vec![
                Method::GET.as_str().to_string(),
                Method::POST.as_str().to_string(),
                Method::PUT.as_str().to_string(),
                Method::DELETE.as_str().to_string(),
                Method::OPTIONS.as_str().to_string(),
            ],
            cors_allowed_headers: vec![
                customized_headers::HEADER_SERVICE_KEY.to_string(),
                customized_headers::HEADER_SOURCE_KEY.to_string(),
                customized_headers::X_FORWARDED_FOR.to_string(),
                customized_headers::X_REAL_IP.to_string(),
                header::CONTENT_TYPE.as_str().to_string(),
                header::COOKIE.as_str().to_string(),
                header::AUTHORIZATION.as_str().to_string(),
                header::CONTENT_LENGTH.as_str().to_string(),
                header::ACCEPT_LANGUAGE.as_str().to_string(),
                header::ACCEPT_RANGES.as_str().to_string(),
                header::ACCEPT.as_str().to_string(),
                header::FORWARDED.as_str().to_string(),
                header::USER_AGENT.as_str().to_string(),
                header::DATE.as_str().to_string(),
                header::AGE.as_str().to_string(),
                header::CACHE_CONTROL.as_str().to_string(),
                header::FORWARDED.as_str().to_string(),
                header::HOST.as_str().to_string(),
                header::X_XSS_PROTECTION.as_str().to_string(),
                header::EXPIRES.as_str().to_string(),
            ],

            forward_roles_header: None,
            forward_subject_header: None,
        }
    }
}

impl Settings {
    fn get_allowed_methods(&self) -> String {
        self.cors_allowed_methods.join(", ")
    }

    fn get_allowed_headers(&self) -> String {
        self.cors_allowed_headers.join(", ")
    }
}

fn get_remote_real_ip(headers: &HeaderMap, client_address: &SocketAddr) -> Option<RealIp> {
    let remote = match client_address {
        SocketAddr::Inet(socket_addr) => socket_addr.ip(),
        // request receive from localhost domain socket
        SocketAddr::Unix(_domain_socket) => return Some(RealIp::DomainSocket),
    };
    let real_ip = real_ip(headers, remote, &[])?;
    Some(RealIp::Remote(real_ip))
}

pub struct HttpProxy {
    pub upstreams: HashMap<ServiceRegistry, Arc<LoadBalancer<RoundRobin>>>,
    pub settings: Settings,
}

impl HttpProxy {
    pub fn new(
        upstreams: HashMap<ServiceRegistry, Arc<LoadBalancer<RoundRobin>>>,
        settings: Settings,
    ) -> Self {
        Self {
            upstreams,
            settings,
        }
    }

    fn localhost_backend(&self) -> Result<Backend> {
        let mut localhost = Backend::new(&format!(
            "{LOCALHOST_IP}:{}",
            self.settings.local_http_server_port
        ))?;
        localhost.set_port(self.settings.local_http_server_port);
        Ok(localhost)
    }

    async fn add_cors_request(&self, session: &mut Session) -> Result<()> {
        if session.req_header().method == Method::OPTIONS {
            if let Some(origin) = session.req_header().headers.get(header::ORIGIN) {
                let origin_str = match origin.to_str() {
                    Ok(origin_str) => origin_str,
                    Err(_) => return Ok(()),
                };
                let allowed_origins = &self.settings.cors_allowed_origins;
                if !allowed_origins.contains("*") && !allowed_origins.contains(origin_str) {
                    return Ok(());
                }

                let mut response_headers = ResponseHeader::build(StatusCode::OK, None)?;
                response_headers.append_header(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin)?;
                response_headers.append_header(
                    header::ACCESS_CONTROL_ALLOW_METHODS,
                    self.settings.get_allowed_methods(),
                )?;
                response_headers.append_header(
                    header::ACCESS_CONTROL_ALLOW_HEADERS,
                    self.settings.get_allowed_headers(),
                )?;
                response_headers.append_header(header::ACCESS_CONTROL_ALLOW_CREDENTIALS, "true")?;
                response_headers
                    .append_header(header::ACCESS_CONTROL_MAX_AGE, ACCESS_CONTROL_MAX_AGE_HOURS)?;
                session
                    .write_response_header(Box::new(response_headers), false)
                    .await?;
            }
        }
        Ok(())
    }

    async fn add_cors_response(
        &self,
        session: &Session,
        upstream_response: &mut ResponseHeader,
    ) -> Result<()> {
        if let Some(origin) = session.req_header().headers.get(header::ORIGIN) {
            let origin_str = match origin.to_str() {
                Ok(origin_str) => origin_str,
                Err(_) => return Ok(()),
            };
            let allowed_origins = &self.settings.cors_allowed_origins;
            if !allowed_origins.contains("*") && !allowed_origins.contains(origin_str) {
                return Ok(());
            }
            upstream_response.append_header(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin)?;
            upstream_response.append_header(header::ACCESS_CONTROL_ALLOW_CREDENTIALS, "true")?;
            if session.req_header().method != Method::OPTIONS {
                upstream_response.append_header(
                    header::ACCESS_CONTROL_ALLOW_HEADERS,
                    self.settings.get_allowed_headers(),
                )?;
            }
        }
        Ok(())
    }

    async fn add_forward_headers_request(
        &self,
        ctx: &ProxyContext,
        session: &mut Session,
    ) -> Result<()> {
        // add roles to header
        if let Some(user_info) = ctx.user_info.as_ref() {
            if let Some(role_header) = self.settings.forward_roles_header.as_ref() {
                let roles = user_info
                    .roles
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                session
                    .req_header_mut()
                    .insert_header(role_header.clone().into_case_header_name(), roles)?;
            }
            if let Some(subject_header) = self.settings.forward_subject_header.as_ref() {
                let user_subject = user_info.user_subject.subject.get_subject_value();
                session
                    .req_header_mut()
                    .insert_header(subject_header.clone().into_case_header_name(), user_subject)?;
            }
        }
        Ok(())
    }

    async fn add_forward_headers_response(
        &self,
        ctx: &ProxyContext,
        upstream_response: &mut ResponseHeader,
    ) -> Result<()> {
        // add roles to header
        if let Some(user_info) = ctx.user_info.as_ref() {
            if let Some(role_header) = self.settings.forward_roles_header.as_ref() {
                let roles = user_info
                    .roles
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                upstream_response
                    .append_header(role_header.clone().into_case_header_name(), roles)?;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl ProxyHttp for HttpProxy {
    /// The per request object to share state across the different filters
    type CTX = ProxyContext;

    /// Define how the `ctx` should be created.
    fn new_ctx(&self) -> Self::CTX {
        ProxyContext::default()
    }

    async fn early_request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<()>
    where
        Self::CTX: Send + Sync,
    {
        let log_request_as_info = session
            .req_header()
            .headers
            .get_ignore_case(customized_headers::X_DEBUG_REQ)
            .map(|value| value.to_str().ok())
            .flatten()
            .map(|debug| {
                let is_debug = match debug.parse::<u8>() {
                    Ok(1) => true,
                    Err(_e) => {
                        let debug_str = debug.to_lowercase();
                        match debug_str.as_str() {
                            "true" | "yes" | "t" | "y" => true,
                            _ => false,
                        }
                    }
                    _ => false,
                };
                is_debug
            })
            .unwrap_or(false);
        ctx.log_request_as_info = log_request_as_info;
        // try to parse service name from request, but this "service name" does not means
        // it is a real service, we should find it from our service table to make sure it
        // exists.
        let service_candidate = ServiceExtractFrom::extract_service_name(session);

        let proxy_svc = factory::get_proxy_svc().await;
        // try to lookup backends of service with candidate name from table
        let service_name_candidate = service_candidate.get_service_name();
        let found_service = match service_name_candidate {
            Some(service_name_candidate) => {
                let service_backends = proxy_svc
                    .get_online_service_backends(service_name_candidate)
                    .await;
                if service_backends.is_empty() {
                    None
                } else {
                    Some((service_name_candidate, service_backends))
                }
            }
            _ => None, // use localhost if service is not present
        };

        // set `redirect_target`
        match found_service {
            Some((service_name, service_backends)) => {
                service_candidate.remove_service_name_from_uri(session)?;
                ctx.redirect_target = RedirectTarget::Service {
                    service_name: service_name.to_string(),
                    service_backends,
                };
            }
            None => ctx.redirect_target = RedirectTarget::Localhost,
        };

        // try to get request source from request header
        let from_platform = session
            .req_header()
            .headers
            .get_ignore_case(customized_headers::HEADER_SOURCE_KEY)
            .map(|value| value.to_str().ok())
            .flatten()
            .filter(|v| !v.trim().is_empty())
            .map(Platform::from_str)
            // otherwise seems as request to gateway (localhost)
            .unwrap_or(Platform::Gateway);
        ctx.request_from = from_platform;

        // try to get client address
        let address = &session.client_addr().ok_or(forbidden())?;
        let real_ip = get_remote_real_ip(&session.req_header().headers, *address);
        ctx.real_ip = real_ip;

        let (to_backend, service_name) = match &ctx.redirect_target {
            RedirectTarget::Service {
                service_backends,
                service_name,
            } => {
                let mut backend = None;
                // try to find an available backend from all kinds registry of this service
                for (registry, backends) in service_backends {
                    if backends.backend_count < 1 {
                        continue;
                    }
                    if let Some(realtime_backends) = self
                        .upstreams
                        .get(registry)
                        .map(|balancer| balancer.backends())
                    {
                        backend = backends.select(realtime_backends);
                    }

                    // if we can find at least one available backend endpoint to use, then
                    // use it immediately.
                    if backend.is_some() {
                        break;
                    }
                }
                match backend {
                    Some(backend) => (backend, Some(service_name.to_string())),
                    None => {
                        // otherwise try localhost
                        (self.localhost_backend()?, None)
                    }
                }
            }
            RedirectTarget::Localhost => (self.localhost_backend()?, None),
        };

        ctx.to_service = service_name;
        ctx.to_backend = Some(to_backend);

        // we must get route config after removing service from uri
        let method = &session.req_header().method;
        let route_config = proxy_svc
            .match_route_config(
                &ctx.request_from,
                &ctx.redirect_target,
                method,
                &session.req_header().uri,
            )
            .await?;
        ctx.route_config = route_config;

        Ok(())
    }

    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool>
    where
        Self::CTX: Send + Sync,
    {
        // step 1: process cross-origin
        if self.settings.allow_cors {
            self.add_cors_request(session).await?;
        }
        let headers = &session.req_header().headers;

        // step 2: start to check the ratelimit of the request if config exists
        if let Some(route_config) = ctx.route_config.as_ref() {
            let client_id = ctx
                .real_ip
                .as_ref()
                .map(|ip| ip.get_client_id(&ctx.redirect_target))
                .unwrap_or("unknown".to_string());
            if !route_config.ratelimit_acquire(client_id, 1) {
                let mut header = ResponseHeader::build(429, None).map_err(|_| internal_error())?;
                let limit = route_config.ratelimit_max_req_per_seconds();
                if let Some(limit) = limit {
                    header
                        .insert_header("RateLimit-Limit", limit)
                        .map_err(|_| internal_error())?;
                    header
                        .insert_header("RateLimit-Remaining", "0")
                        .map_err(|_| internal_error())?;
                    header
                        .insert_header("RateLimit-Reset", "1")
                        .map_err(|_| internal_error())?;
                }
                session.set_keepalive(None);
                session
                    .write_response_header(Box::new(header), true)
                    .await?;
                return Ok(true);
            }
        }

        // step 3: try to get user info from header.
        let authentication_svc = factory::get_authentication_svc().await;
        let unchecked_user_info = authentication_svc
            .get_user_info(&ctx.request_from, headers)
            .await?;
        ctx.user_info = unchecked_user_info;

        // step 4: check logged in user identity
        if let Some(user_info) = ctx.user_info.as_ref() {
            let is_valid_user = authentication_svc
                .verify_user_identity(user_info, headers)
                .await
                .is_ok();
            ctx.is_valid_user = Some(is_valid_user);
        }

        if ctx.log_request_as_info {
            tracing::info!("request context: {ctx:?}");
        } else {
            tracing::debug!("request context: {ctx:?}");
        }

        if ctx.is_valid_user == Some(false) {
            // if user token exist but is invalid, then forbidden to access.
            return Err(anyhow!("Invalid user"))
                .log_if_error(Level::DEBUG)
                .map_err(|_| forbidden())?;
        }

        // step 5: try to get url corresponding configuration
        let route_config = ctx.route_config.as_ref().cloned();

        // if config of route does not exists but we allow to access internal services.
        // then allow this request goes on.
        if route_config.is_none()
            // request target is service
            && ctx.redirect_target.is_service()
            // enable to access unconfigured api
            && self.settings.allow_unconfigured_api_exposed
        {
            // then allow to access
            return Ok(false);
        }

        let route_config = route_config
            .context("Failed to get route config")
            .map_err(|_| forbidden())?;

        // step 6: check the request if allow to access the url.
        let allow_anonymous_to_access =
            route_config.is_allowed_roles(&ctx.request_from, &[Role::Anonymous].into());
        let mut allow_user_to_access = false;

        if let Some(user_info) = ctx.user_info.as_ref() {
            authentication_svc
                .verify_user_access_privilege(route_config.as_ref(), &ctx.request_from, &user_info)
                .await?;
            // after check privilege, we set `allow_user_to_access` to true.
            allow_user_to_access = true;
        }

        let allow_request_access = allow_anonymous_to_access || allow_user_to_access;
        if !allow_request_access {
            return Err(anyhow!("Invalid request")).map_err(|_| forbidden())?;
        }
        Ok(false)
    }

    /// Define where the proxy should send the request to.
    ///
    /// The returned [HttpPeer] contains the information regarding where and how this request should
    /// be forwarded to.
    async fn upstream_peer(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let _ = self
            .add_forward_headers_request(ctx, session)
            .await
            .context("Failed to add forward headers to upstream request")
            .log_if_error(Level::ERROR);
        let to_backend = ctx
            .to_backend
            .as_ref()
            .context("Failed to get target backend")
            .map_err(|_| notfound())?
            .clone();
        if ctx.log_request_as_info {
            tracing::info!(
                "handle request {}:{}",
                session.req_header().method,
                session.req_header().uri
            );
        } else {
            tracing::debug!(
                "handle request {}:{}",
                session.req_header().method,
                session.req_header().uri
            );
        }

        let mut peer = Box::new(HttpPeer::new(
            to_backend,
            false,
            self.settings.sni.to_owned(),
        ));
        if let Some(connection_timeout) = self.settings.allowed_connection_timeout {
            peer.options.connection_timeout = Some(connection_timeout);
        }
        if let Some(read_timeout) = self.settings.allowed_read_timeout {
            peer.options.read_timeout = Some(read_timeout);
        }
        if let Some(write_timeout) = self.settings.allowed_write_timeout {
            peer.options.write_timeout = Some(write_timeout);
        }
        Ok(peer)
    }

    async fn response_filter(
        &self,
        session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()>
    where
        Self::CTX: Send + Sync,
    {
        if self.settings.allow_cors {
            self.add_cors_response(session, upstream_response).await?;
        }
        self.add_forward_headers_response(ctx, upstream_response)
            .await?;
        Ok(())
    }

    fn fail_to_connect(
        &self,
        _session: &mut Session,
        _peer: &HttpPeer,
        ctx: &mut Self::CTX,
        mut e: Box<Error>,
    ) -> Box<Error> {
        let max_retry_times = ctx
            .route_config
            .as_ref()
            .map(|config| config.allowed_retry_times)
            .flatten()
            .unwrap_or(0);
        if ctx.tries >= max_retry_times {
            return e;
        }
        ctx.tries += 1;
        e.set_retry(true);
        e
    }
}
