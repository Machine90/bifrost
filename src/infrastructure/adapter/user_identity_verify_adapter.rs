use anyhow::{Context, Result};
use http::{HeaderMap, Method};
use url::Url;

#[derive(Debug, Clone)]
pub(crate) struct UserIdentityVerifyHttpClient {
    method: Method,
    http_client: reqwest::Client,
}

impl UserIdentityVerifyHttpClient {
    pub(crate) fn new(method: Method, http_client: reqwest::Client) -> Self {
        Self {
            method,
            http_client,
        }
    }

    pub(crate) async fn verify_user_identity(
        &self,
        url: Url,
        forwarded_headers: &HeaderMap,
    ) -> Result<bool> {
        let status = self
            .http_client
            .request(self.method.clone(), url)
            .headers(forwarded_headers.clone())
            .send()
            .await
            .context("Failed to request to target")?
            .status();
        Ok(status.is_success())
    }
}
