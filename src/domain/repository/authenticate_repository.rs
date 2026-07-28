use anyhow::Result;
use http::HeaderMap;

use crate::domain::model::entity::user_info::UserBaseInfo;

#[async_trait::async_trait]
pub trait AuthenticateRepository: Send + Sync + 'static {
    async fn verify_user_identity(
        &self,
        user_info: &UserBaseInfo,
        forwarded_request_headers: &HeaderMap,
    ) -> Result<()>;
}
