pub(crate) mod customized_headers {
    /// Request received from which platform, if this header is missing, then default
    /// to unknown.
    pub(crate) const HEADER_SOURCE_KEY: &str = "X-Req-From";
    pub(crate) const HEADER_SERVICE_KEY: &str = "X-Req-Service";

    pub(crate) const X_FORWARDED_FOR: &str = "x-forwarded-for";
    pub(crate) const X_REAL_IP: &str = "x-real-ip";

    pub(crate) const X_DEBUG_REQ: &str = "X-Req-Debug";
}

pub(crate) mod backend {
    pub(crate) const WEIGHT_SCALE: f64 = 10_000.;
}

pub(crate) mod http_server {
    pub(crate) const DEFAULT_HTTP_PORT: u16 = 8080;

    pub(crate) const BASE_URL: &str = "/api/v1/gateway";
    pub(crate) const OPENAPI_URL: &str = constcat::concat!("/gateway/api/v1", "/openapi.json");
}

pub(crate) mod http_proxy {
    pub(crate) const DEFAULT_SNI: &str = "one.one.one.one";

    pub(crate) const LOCALHOST_SERVICE_NAME: &str = "localhost";
}

pub(crate) mod dao {
    pub(crate) const FETCH_PRIVILEGE_PAGE_SIZE: usize = 100;
    pub(crate) const FETCH_USER_PAGE_SIZE: usize = 200;
}

pub(crate) mod mock {
    pub(crate) const MOCKED_OPERATE_USER_ID: &str = "1";
    pub(crate) const MOCKED_CONFIG_VERSION: i32 = 1;
}
