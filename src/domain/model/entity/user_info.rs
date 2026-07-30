use std::collections::HashSet;

use anyhow::{Context, Result};
use http::{
    HeaderMap,
    header::{AUTHORIZATION, COOKIE},
};

use crate::{
    common::error_types::ErrorKind,
    domain::model::value::{
        authorization_header::AuthorizationHeader,
        role::Role,
        subject::Subject,
        tokens::{Tokens, TokensKind},
    },
};

#[derive(Debug, Clone)]
pub struct UserSubject {
    pub tokens_kind: TokensKind,
    pub subject: Subject,
}

#[derive(Debug, Clone)]
pub struct UserBaseInfo {
    pub user_subject: UserSubject,
    pub roles: HashSet<Role>,
}

impl UserSubject {
    pub fn from_header(
        headers: &HeaderMap,
        access_token_name: Option<&str>,
        refresh_token_name: Option<&str>,
    ) -> Result<Option<UserSubject>> {
        let user_cookie = headers.get(COOKIE).map(|v| v.to_str().ok()).flatten();
        let auth_token = headers
            .get(AUTHORIZATION)
            .map(|v| v.to_str().ok())
            .flatten();

        // first priority to extract user subject from cookie.
        let mut tokens = match (user_cookie, access_token_name) {
            (Some(user_cookie), Some(_)) => {
                let tokens =
                    Tokens::from_cookies(user_cookie, access_token_name, refresh_token_name);
                if tokens.is_empty() {
                    None
                } else {
                    Some(tokens)
                }
            }
            _ => None,
        };

        // otherwise extract user subject from authorization header if cookie is missing.
        if tokens.is_none() && auth_token.is_some() {
            let auth_header = AuthorizationHeader::try_from(headers)
                .context(ErrorKind::Forbidden)
                .context("Failed to parse auth token from header")?;
            tokens = Some(
                Tokens::from_auth_header(auth_header)
                    .context(ErrorKind::Forbidden)
                    .context("Failed to parse JWT from Authorization")?,
            );
        }

        let tokens = match tokens {
            Some(tokens) => tokens,
            None => return Ok(None),
        };
        let tokens_kind = TokensKind::from(&tokens);
        let subject = Subject::try_from(&tokens);
        let subject = match subject {
            Ok(subject) => subject,
            // failed to parse subject from token.
            Err(_) => return Ok(None),
        };
        Ok(Some(Self {
            tokens_kind,
            subject,
        }))
    }
}
