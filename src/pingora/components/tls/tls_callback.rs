use async_trait::async_trait;
use pingora::listeners::TlsAccept;

pub struct TlsCallback;

#[async_trait]
impl TlsAccept for TlsCallback {}
