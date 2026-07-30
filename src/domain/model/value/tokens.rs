use anyhow::{Context, Result};
use cookie::{Cookie, time::OffsetDateTime};
use enum_as_inner::EnumAsInner;
use enum_kinds::EnumKind;
use jsonwebtoken::{TokenData, dangerous::insecure_decode};
use serde::{Deserialize, Serialize};

use crate::domain::model::value::authorization_header::AuthorizationHeader;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtTokenPayload {
    #[serde(deserialize_with = "crate::common::serde_helper::deserialize_option_str")]
    pub sub: Option<String>,
    pub iat: Option<u64>,
    pub exp: Option<u64>,
}

impl JwtTokenPayload {
    pub fn get_expiration(&self) -> anyhow::Result<Option<OffsetDateTime>> {
        let exp = match self.exp {
            Some(exp) => exp as _,
            _ => return Ok(None),
        };
        let datetime = OffsetDateTime::from_unix_timestamp(exp)?;
        Ok(Some(datetime))
    }

    pub fn get_issue_at(&self) -> anyhow::Result<Option<OffsetDateTime>> {
        let iat = match self.iat {
            Some(iat) => iat as _,
            _ => return Ok(None),
        };
        let datetime = OffsetDateTime::from_unix_timestamp(iat)?;
        Ok(Some(datetime))
    }
}

#[derive(Debug, Clone)]
pub struct JwtToken {
    pub token_data: TokenData<JwtTokenPayload>,
    pub raw_token: String,
}

impl JwtToken {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        let token_data = insecure_decode::<JwtTokenPayload>(value)?;
        Ok(Self {
            token_data,
            raw_token: value.to_string(),
        })
    }

    pub fn is_expired(&self) -> anyhow::Result<bool> {
        let is_expired = self
            .token_data
            .claims
            .get_expiration()?
            .map(|dt| dt < OffsetDateTime::now_utc())
            // not expiration specified means never expires
            .unwrap_or(false);
        Ok(is_expired)
    }
}

#[derive(Debug, Clone, EnumKind)]
#[enum_kind(TokensKind, derive(EnumAsInner))]
pub enum Tokens {
    PlaintextFromAuthHeader {
        subject: String,
    },
    JwtTokensFromCookies {
        access_token: Option<JwtToken>,
        refresh_token: Option<JwtToken>,
    },
    JwtTokenFromAuthHeader {
        auth_token: JwtToken,
    },
}

impl Tokens {
    pub fn is_empty(&self) -> bool {
        match self {
            Tokens::PlaintextFromAuthHeader { subject } => subject.is_empty(),
            Tokens::JwtTokensFromCookies {
                access_token,
                refresh_token,
            } => access_token.is_none() && refresh_token.is_none(),
            Tokens::JwtTokenFromAuthHeader { auth_token } => auth_token.raw_token.is_empty(),
        }
    }

    pub fn from_auth_header(auth_header: AuthorizationHeader) -> Result<Tokens> {
        let tokens = match auth_header {
            AuthorizationHeader::Basic { username, .. } => {
                Tokens::PlaintextFromAuthHeader { subject: username }
            }
            AuthorizationHeader::Bearer { token } => {
                if let Ok(auth_token) = JwtToken::parse(&token) {
                    // try to parse token as JWT, and set it as jwt if success
                    Tokens::JwtTokenFromAuthHeader { auth_token }
                } else {
                    // otherwise we don't support this kind of token, maybe support
                    // it in the future
                    return Err(anyhow::anyhow!(
                        "Failed to parse auth token, only support jwt for now"
                    ))
                    .context(format!("token = {token:?}"))
                    .context("Failed to parse auth token")?;
                }
            }
        };
        Ok(tokens)
    }

    pub fn from_cookies(
        user_cookie: &str,
        access_token_name: Option<&str>,
        refresh_token_name: Option<&str>,
    ) -> Tokens {
        let cookies = Cookie::split_parse(user_cookie);
        let user_tokens = cookies
            .into_iter()
            .filter_map(|c| c.ok())
            .filter_map(|user_cookie| {
                // check if cookie is valid
                let expired = user_cookie
                    .expires()
                    .map(|e| e.datetime())
                    .flatten()
                    .map(|expires| expires < OffsetDateTime::now_utc())
                    .unwrap_or(false);
                if expired {
                    return None;
                }
                Some(user_cookie)
            })
            .fold(
                Tokens::JwtTokensFromCookies {
                    access_token: None,
                    refresh_token: None,
                },
                |mut tokens, user_cookie| {
                    let mut is_access_token = false;
                    let mut is_refresh_token = false;
                    if let Some(key) = access_token_name.as_ref() {
                        is_access_token = user_cookie.name() == *key;
                    }
                    if let Some(key) = refresh_token_name.as_ref() {
                        is_refresh_token = user_cookie.name() == *key;
                    }
                    if is_access_token {
                        let _r = tokens
                            .add_cookie_access_token(user_cookie)
                            .context("Failed to parse jwt token ");
                    } else if is_refresh_token {
                        let _r = tokens
                            .add_cookie_refresh_token(user_cookie)
                            .context("Failed to parse jwt token ");
                    }
                    tokens
                },
            );
        user_tokens
    }

    pub fn add_cookie_access_token(&mut self, cookie: Cookie<'_>) -> Result<()> {
        let value = cookie.value();
        let token = JwtToken::parse(value)?;
        match self {
            Tokens::JwtTokensFromCookies { access_token, .. } => {
                *access_token = Some(token);
            }
            _ => (),
        };
        Ok(())
    }

    pub fn add_cookie_refresh_token(&mut self, cookie: Cookie<'_>) -> Result<()> {
        let value = cookie.value();
        let token = JwtToken::parse(value)?;
        match self {
            Tokens::JwtTokensFromCookies { refresh_token, .. } => {
                *refresh_token = Some(token);
            }
            _ => (),
        };
        Ok(())
    }
}
