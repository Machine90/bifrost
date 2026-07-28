use std::{borrow::Borrow, fmt::Debug};

use base64::{Engine as _, prelude::BASE64_STANDARD};
use http::{HeaderMap, HeaderValue, header::InvalidHeaderValue};

/// [Authorization header](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Authorization)
#[derive(Debug)]
pub enum AuthorizationHeader {
    /// [Basic auth header](https://datatracker.ietf.org/doc/html/rfc7617)
    Basic {
        /// Basic auth username
        username: String,
        /// Basic auth password
        password: String,
    },
    /// [Bearer token](https://datatracker.ietf.org/doc/html/rfc6750)
    Bearer {
        /// Bearer token
        token: String,
    },
}

impl TryFrom<HeaderMap> for AuthorizationHeader {
    type Error = anyhow::Error;

    fn try_from(value: HeaderMap) -> Result<Self, Self::Error> {
        let auth_head_parts: Vec<&str> = value
            .get(http::header::AUTHORIZATION)
            .ok_or(anyhow::anyhow!("Auth header is missing"))?
            .to_str()
            .map_err(|_| anyhow::anyhow!("Invalid characters"))?
            .split(' ')
            .collect();

        let (auth_type, auth_content) = auth_head_parts
            .split_first()
            .ok_or(anyhow::anyhow!("Bad format"))?;

        match (auth_type.to_lowercase().as_str(), auth_content) {
            ("basic", [auth_content, ..]) => Ok(Self::parse_basic_auth(auth_content)?),
            ("bearer", [auth_content, ..]) => Ok(Self::Bearer {
                token: (*auth_content).to_string(),
            }),
            (auth_type, [..]) => Err(anyhow::anyhow!("Unknown auth type {:?}", auth_type)),
        }
    }
}

impl TryFrom<&HeaderMap> for AuthorizationHeader {
    type Error = anyhow::Error;

    fn try_from(value: &HeaderMap) -> Result<Self, Self::Error> {
        value.to_owned().try_into()
    }
}

impl TryFrom<&AuthorizationHeader> for HeaderMap {
    type Error = anyhow::Error;
    fn try_from(auth: &AuthorizationHeader) -> Result<Self, Self::Error> {
        let mut headers = HeaderMap::new();
        headers.insert(http::header::AUTHORIZATION, auth.header_value()?);
        Ok(headers)
    }
}

impl TryFrom<AuthorizationHeader> for HeaderMap {
    type Error = anyhow::Error;
    fn try_from(auth: AuthorizationHeader) -> Result<Self, Self::Error> {
        auth.borrow().try_into()
    }
}

impl AuthorizationHeader {
    /// Constructor for basic authorization header
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self::Basic {
            username: username.into(),
            password: password.into(),
        }
    }

    /// Constructor for bearer authorization header
    pub fn bearer(token: impl Into<String>) -> Self {
        Self::Bearer {
            token: token.into(),
        }
    }

    /// parse the basic auth token
    fn parse_basic_auth(auth: &str) -> anyhow::Result<Self> {
        let auth_string = String::from_utf8(BASE64_STANDARD.decode(auth)?)?;
        let (username, password) = auth_string
            .split_once(':')
            .ok_or(anyhow::anyhow!("DelimiterNotFound"))?;
        Ok(Self::Basic {
            username: username.to_owned(),
            password: password.to_owned(),
        })
    }

    /// generate a HeaderValue
    /// ```
    /// # use auth_headers::AuthorizationHeader;
    ///
    /// let header = AuthorizationHeader::basic("aladdin", "opensesame");
    /// assert_eq!(
    ///     http::HeaderValue::from_str("Basic YWxhZGRpbjpvcGVuc2VzYW1l").unwrap(),
    ///     header.header_value().unwrap()
    /// );
    /// ```
    pub fn header_value(&self) -> Result<HeaderValue, InvalidHeaderValue> {
        let value = match self {
            Self::Basic { username, password } => {
                format!(
                    "Basic {}",
                    BASE64_STANDARD.encode(format!("{}:{}", username, password))
                )
            }
            Self::Bearer { token } => format!("Bearer {}", token),
        };
        value.parse()
    }
}
