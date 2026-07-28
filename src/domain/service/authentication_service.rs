use std::{collections::HashSet, sync::Arc};

use anyhow::Context;
use http::{
    HeaderMap,
    header::{AUTHORIZATION, COOKIE},
};
use partialdebug::placeholder::PartialDebug;

use crate::{
    common::pingora_errors::{forbidden, internal_error},
    domain::{
        model::{
            entity::{route_config::RouteConfig, user_info::UserBaseInfo},
            value::{
                authorization_header::AuthorizationHeader,
                platform::Platform,
                role::Role,
                subject::Subject,
                tokens::{Tokens, TokensKind},
            },
        },
        repository::{
            authenticate_repository::AuthenticateRepository, user_repository::UserRepository,
        },
    },
    settings::Settings,
};

#[derive(PartialDebug, Clone)]
pub struct AuthenticationService {
    user_repo: Arc<dyn UserRepository>,
    authenticate_repo: Arc<dyn AuthenticateRepository>,
}

impl AuthenticationService {
    pub async fn new(
        user_repo: Arc<dyn UserRepository>,
        authenticate_repo: Arc<dyn AuthenticateRepository>,
    ) -> Self {
        Self {
            user_repo,
            authenticate_repo,
        }
    }

    pub async fn get_user_info(
        &self,
        request_source: &Platform,
        headers: &HeaderMap,
    ) -> pingora::Result<Option<UserBaseInfo>> {
        let user_cookie = headers.get(COOKIE).map(|v| v.to_str().ok()).flatten();
        let auth_token = headers
            .get(AUTHORIZATION)
            .map(|v| v.to_str().ok())
            .flatten();
        let settings = Settings::get();
        let access_token_name = settings.auth_args.svc_cookie_access_token_key.as_ref();
        let refresh_token_name = settings.auth_args.svc_cookie_refresh_token_key.as_ref();

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

        if tokens.is_none() && auth_token.is_some() {
            let auth_header = AuthorizationHeader::try_from(headers)
                .context("Failed to parse auth token from header")
                .map_err(|_| forbidden())?;
            tokens = Some(Tokens::from_auth_header(auth_header)?);
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
        let roles = self.get_user_roles(request_source, &subject).await?;
        Ok(Some(UserBaseInfo {
            tokens_kind,
            user_subject: subject,
            roles,
        }))
    }

    pub async fn verify_user_access_privilege(
        &self,
        route_config: &RouteConfig,
        request_source: &Platform,
        user_info: &UserBaseInfo,
    ) -> pingora::Result<()> {
        let UserBaseInfo { roles, .. } = user_info;
        let allow_user_access = self
            .is_user_allow_access_resource(request_source, route_config, &roles)
            .await?;
        if !allow_user_access {
            return Err(forbidden());
        }
        Ok(())
    }

    /// Verify request with it headers, the headers contained some information such as
    /// cookie, token etc, the verify method should be provided by user repository and its
    /// adapter (e.g. an adapter can be a remote user center client).
    pub async fn verify_user_identity(
        &self,
        user_info: &UserBaseInfo,
        request_headers: &HeaderMap,
    ) -> pingora::Result<()> {
        self.authenticate_repo
            .verify_user_identity(user_info, request_headers)
            .await
            .map_err(|_| forbidden())?;
        Ok(())
    }

    async fn get_user_roles(
        &self,
        request_source: &Platform,
        subject: &Subject,
    ) -> pingora::Result<HashSet<Role>> {
        let user_subject = subject.get_subject();
        let user = self
            .user_repo
            .get_user_config(user_subject)
            .await
            .map_err(|_| internal_error())?;
        let roles = match user.map(|config| config.roles(request_source)).flatten() {
            Some(mut user_roles) => {
                user_roles.insert(Role::Untagged);
                user_roles.insert(Role::Anonymous);
                user_roles
            }
            None => [Role::Untagged, Role::Anonymous].into(),
        };
        Ok(roles)
    }

    async fn is_user_allow_access_resource(
        &self,
        request_source: &Platform,
        route_config: &RouteConfig,
        user_roles: &HashSet<Role>,
    ) -> pingora::Result<bool> {
        let allowed = route_config.is_allowed_roles(request_source, &user_roles);
        Ok(allowed)
    }
}
