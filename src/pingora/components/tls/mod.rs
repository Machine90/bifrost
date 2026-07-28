use anyhow::Context;
use openssl::ssl::SslFiletype;
use pingora::listeners::tls::TlsSettings;
use tracing::Level;

use crate::{
    common::tracing_ext::TracingResultExt, pingora::components::tls::tls_callback::TlsCallback,
    settings::Settings,
};

pub(crate) mod tls_callback;

pub fn get_tls_settings() -> Option<TlsSettings> {
    let settings = Settings::get();
    let tls_args = settings.tls_args.clone();

    let mut tls_settings =
        TlsSettings::with_callbacks(Box::new(TlsCallback)).expect("Failed to create TSL settings");
    tls_settings
        .set_certificate_chain_file(tls_args.cert_pem_path?)
        .expect("Invalid TLS cert file");
    tls_settings
        .set_private_key_file(
            tls_args
                .priv_key_pem_path
                .context("Private key pem is missing")
                .log_if_error(Level::WARN)
                .ok()?,
            SslFiletype::PEM,
        )
        .expect("Invalid TLS private key file");
    tls_settings.set_verify(tls_args.verify_mode.to_ssl_verify_mode());
    Some(tls_settings)
}
