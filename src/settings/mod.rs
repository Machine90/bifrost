use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, OnceLock},
};

use anyhow::{Context, Result};
use clap::{ArgAction, CommandFactory, Parser};
use http::Method;
use openssl::ssl::SslVerifyMode;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::settings::service_conf::{ServiceConf, read_conf_dir};

pub mod service_conf;

#[derive(Debug, Clone, Parser, Serialize, Deserialize)]
#[clap(about)]
pub struct Args {
    /// Current active environment, default to 'dev'
    #[arg(short, long, env, default_value = "dev")]
    pub env: String,
    /// Gateway configuration management http server's port
    #[arg(long, alias = "gms_port", env, default_value = "8080")]
    #[serde(alias = "gms_port")]
    pub gateway_management_server_port: u16,
    /// Gateway management platform initial admin user ids
    #[arg(long, alias = "gma_ids", env, default_values_t = [format!("2")], required = false)]
    #[serde(alias = "gma_ids")]
    pub initial_gateway_admin_ids: Vec<String>,
    // Proxy server ports (excludes 443), e.g. 80, 8000, default to 8000
    #[arg(long, alias = "ports", env, default_values_t = [8000])]
    #[serde(alias = "ports")]
    pub proxy_ports: Vec<u16>,
    #[arg(long, alias = "sni", env, default_value = "one.one.one.one")]
    #[serde(alias = "sni")]
    pub server_name_indication: String,
    // TODO: for security purpose, default to false, notes if this value set to true,
    // any request can access internal service.
    #[arg(alias = "api_exposed", long, env, default_value = "true")]
    #[serde(alias = "api_exposed")]
    pub allow_unconfigured_api_exposed: bool,

    /// The path to the configuration file.
    ///
    /// See [`ServerConf`] for more details of the configuration file.
    #[clap(alias = "pingora-conf", short, long, help = "The path to the pingora configuration file.", long_help = None)]
    #[serde(alias = "pingora-conf", default)]
    pub pingora_conf_path: Option<PathBuf>,

    /// The static filter config file path, this path can be directory or file,
    /// the privilege rules of service will be loaded into gateway if specified.
    #[clap(alias = "filter_conf", short, long, help = "The path to the static route configuration file or root dir.", long_help = None)]
    #[serde(alias = "filter_conf", default)]
    pub static_filter_conf_path: Option<PathBuf>,

    #[command(flatten)]
    pub auth_args: AuthArgs,

    #[command(flatten)]
    #[serde(default)]
    pub tls_args: TlsArgs,

    #[command(flatten)]
    #[serde(default)]
    pub cors_args: CorsArgs,

    #[command(flatten)]
    #[serde(default)]
    pub sentry_args: SentryArgs,

    /// If use nacos as service discovery, these args must be set
    #[cfg(feature = "nacos")]
    #[command(flatten)]
    #[serde(default)]
    pub nacos_args: Option<NacosArgs>,

    #[command(flatten)]
    pub db_args: DatabaseArgs,
}

#[derive(Debug, Clone, Parser, Serialize, Deserialize)]
#[command(about)]
pub struct AuthArgs {
    #[arg(alias = "access_token", long, env)]
    #[serde(alias = "access_token")]
    pub svc_cookie_access_token_key: Option<String>,
    #[arg(alias = "refresh_token", long, env)]
    #[serde(alias = "refresh_token")]
    pub svc_cookie_refresh_token_key: Option<String>,
    #[arg(alias = "roles_header", long, env, default_value = "X-Bifrost-Roles")]
    #[serde(default, alias = "roles_header")]
    pub forward_roles_header: Option<String>,
    #[arg(
        alias = "subject_header",
        long,
        env,
        default_value = "X-Bifrost-Subject"
    )]
    #[serde(default, alias = "subject_header")]
    pub forward_subject_header: Option<String>,

    /// The `user_identity_verify_url` was used to request to target server to
    /// complete user identity verify process, this value should contains information
    /// of method and url, e.g. "get:https://localhost/user/check", if not method specified
    /// then you will get an error.
    #[arg(alias = "verify_url", long, env)]
    #[serde(alias = "verify_url")]
    pub user_identity_verify_url: Option<String>,
}

impl AuthArgs {
    pub fn get_user_identity_verify_url(&self) -> Result<Option<(Method, Url)>> {
        let method_url = self.user_identity_verify_url.as_ref();
        let method_url = match method_url {
            Some(method_url) => method_url.to_string(),
            None => return Ok(None),
        };
        let method_candidate = method_url.split(":").next().context("Invalid url")?;
        let method = Method::from_str(&method_candidate.to_uppercase())?;
        let url_candidate = method_url
            .strip_prefix(&format!("{method_candidate}:"))
            .context("Invalid url")?;
        let url = Url::parse(url_candidate)?;
        Ok(Some((method, url)))
    }
}

#[derive(Debug, Default, Clone, Parser, Serialize, Deserialize)]
#[command(about)]
pub enum VerifyMode {
    /// Verifies that the peer's certificate is trusted.
    ///
    /// On the server side, this will cause OpenSSL to request a certificate from the client.
    Peer,
    /// Disables verification of the peer's certificate.
    ///
    /// On the server side, this will cause OpenSSL to not request a certificate from the
    /// client. On the client side, the certificate will be checked for validity, but the
    /// negotiation will continue regardless of the result of that check.
    #[default]
    None,
    /// On the server side, abort the handshake if the client did not send a certificate.
    ///
    /// This should be paired with `SSL_VERIFY_PEER`. It has no effect on the client side.
    FailIfNoPeerCert,
}

impl VerifyMode {
    pub fn to_ssl_verify_mode(&self) -> SslVerifyMode {
        match self {
            VerifyMode::Peer => SslVerifyMode::PEER,
            VerifyMode::None => SslVerifyMode::NONE,
            VerifyMode::FailIfNoPeerCert => SslVerifyMode::FAIL_IF_NO_PEER_CERT,
        }
    }
}

impl ToString for VerifyMode {
    fn to_string(&self) -> String {
        match self {
            VerifyMode::Peer => "Peer",
            VerifyMode::None => "None",
            VerifyMode::FailIfNoPeerCert => "FailIfNoPeerCert",
        }
        .to_string()
    }
}

impl FromStr for VerifyMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let mode = s.to_lowercase();
        let mode = match mode.as_str() {
            "peer" => Self::Peer,
            "failifnopeercert" | "finpc" | "fail" => Self::FailIfNoPeerCert,
            _ => Self::None,
        };
        Ok(mode)
    }
}

#[derive(Debug, Default, Clone, Parser, Serialize, Deserialize)]
#[command(about)]
pub struct TlsArgs {
    #[arg(alias = "cert", long, env)]
    #[serde(alias = "cert")]
    pub cert_pem_path: Option<PathBuf>,
    #[arg(alias = "priv_key", long, env)]
    #[serde(alias = "priv_key")]
    pub priv_key_pem_path: Option<PathBuf>,
    #[arg(alias = "verify", long, env, default_value_t = VerifyMode::None)]
    #[serde(default, alias = "verify")]
    pub verify_mode: VerifyMode,
}

#[derive(Debug, Clone, Parser, Serialize, Deserialize)]
#[command(about)]
pub struct CorsArgs {
    #[arg(long, env, default_value = "true", action = ArgAction::SetTrue)]
    pub allow_cors: bool,
}

#[derive(Debug, Default, Clone, Parser, Serialize, Deserialize)]
#[command(about)]
pub struct SentryArgs {
    #[arg(long, env)]
    pub dsn: Option<String>,
}

impl Default for CorsArgs {
    fn default() -> Self {
        Self { allow_cors: true }
    }
}

#[derive(Debug, Clone, Parser, Serialize, Deserialize)]
#[command(about)]
pub struct NacosArgs {
    #[arg(long, env)]
    pub nacos_server_address: String,
    #[arg(long, env)]
    pub nacos_username: String,
    #[arg(long, env)]
    pub nacos_password: String,
    /// Default to 60 secs
    #[arg(short, long, env, default_value = "60")]
    pub cache_service_names_secs: u32,
}

#[derive(Debug, Clone, Parser, Serialize, Deserialize)]
#[command(about)]
pub struct DatabaseArgs {
    #[arg(long, env)]
    pub database_url: String,
}

impl Args {
    pub fn check(&self) {
        if let Some(conf_dir) = self.pingora_conf_path.as_ref() {
            if !conf_dir.exists() || !conf_dir.is_file() {
                panic!("Invalid pingora conf file path");
            }
        }
        let verify_url = self.auth_args.get_user_identity_verify_url();
        if let Err(e) = verify_url {
            panic!("Invalid `user_identity_verify_url`, see: {:?}", e);
        }
    }

    pub fn get_static_privilege_conf(&self) -> ServiceConf {
        let static_path = self.static_filter_conf_path.clone();
        static SINGLETON: OnceLock<ServiceConf> = OnceLock::new();
        let service_conf = SINGLETON
            .get_or_init(move || {
                let mut result = None;
                if let Some(path) = static_path {
                    let service_conf = read_conf_dir(path).expect("msg");
                    result = Some(service_conf);
                }
                result.unwrap_or_default()
            })
            .clone();
        service_conf
    }
}

#[derive(Debug, Clone, Parser, Serialize, Deserialize)]
#[clap(about)]
pub enum Input {
    Config {
        #[arg(default_value = "./config.toml")]
        conf: PathBuf,
    },
    Args {
        #[command(flatten)]
        args: Args,
    },
}

pub static SINGLETON: OnceLock<Arc<Args>> = OnceLock::new();

pub struct Settings;

impl Settings {
    pub fn get() -> Arc<Args> {
        let args = SINGLETON
            .get_or_init(|| {
                let args = if cfg!(debug_assertions) {
                    let args = local_args().expect("Failed to read args");
                    args
                } else {
                    let args = release_args().expect("Failed to read args");
                    args
                };
                Arc::new(args)
            })
            .clone();
        args
    }
}

fn release_args() -> anyhow::Result<Args> {
    let parsed_args = read_args("config.toml")?;
    Ok(parsed_args)
}

fn local_args() -> anyhow::Result<Args> {
    let parsed_args = read_args("./build/local/config.toml")?;
    Ok(parsed_args)
}

fn read_args<P: AsRef<Path>>(default_config_path: P) -> anyhow::Result<Args> {
    let input = Input::try_parse();
    let input = match input {
        Ok(input) => input,
        Err(_e) => {
            eprintln!("Input is missing, use default 'config.toml' file as input.");
            let _ = Input::command().print_help()?;
            Input::Config {
                conf: default_config_path.as_ref().to_path_buf(),
            }
        }
    };
    let mut parsed_args = None;
    match input {
        Input::Config { conf } => {
            if std::fs::exists(&conf).unwrap_or(false) {
                let config_content = std::fs::read_to_string(&conf)?;
                let config = toml::from_str::<Args>(&config_content);
                if let Some(e) = config.as_ref().err() {
                    eprintln!("Failed to parse config from {conf:?}, use args;\n{e:?}");
                }
                parsed_args = config.ok();
            } else {
                eprintln!("Configure file {conf:?} is missing.");
            }
        }
        Input::Args { args } => {
            parsed_args = Some(args);
        }
    };
    parsed_args.context("Failed to parse args")
}
