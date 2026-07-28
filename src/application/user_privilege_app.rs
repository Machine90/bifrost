use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use http::{HeaderMap, header::COOKIE};

use crate::{
    common::error_types::ErrorKind,
    domain::{
        model::{
            entity::privilege_rule::PrivilegeRule,
            value::{platform::Platform, role::Role, subject::Subject, tokens::Tokens},
        },
        service::{
            cluster_manage_service::ClusterManageService, user_manage_service::UserManageService,
        },
    },
    settings::Settings,
};

#[derive(Debug, Clone)]
pub struct UserPrivilegeApp {
    user_manage_svc: UserManageService,
    cluster_manage_svc: ClusterManageService,
}

impl UserPrivilegeApp {
    pub fn new(
        user_manage_svc: UserManageService,
        cluster_manage_svc: ClusterManageService,
    ) -> Self {
        Self {
            user_manage_svc,
            cluster_manage_svc,
        }
    }

    pub async fn get_user_roles(
        &self,
        headers: HeaderMap,
    ) -> Result<(Option<String>, HashMap<Platform, HashSet<Role>>)> {
        let user_cookie = headers.get(COOKIE).map(|v| v.to_str().ok()).flatten();
        let mut current_user_id = None;
        let user_roles = match user_cookie {
            None => [(Platform::Gateway, [Role::Anonymous].into())].into(),
            Some(user_cookie) => {
                let settings = Settings::get();
                let access_token_name = settings.auth_args.svc_cookie_access_token_key.as_ref();
                let refresh_token_name = settings.auth_args.svc_cookie_refresh_token_key.as_ref();
                let tokens =
                    Tokens::from_cookies(user_cookie, access_token_name, refresh_token_name);
                let sub = Subject::try_from(&tokens).context(ErrorKind::Forbidden)?;
                let user_id = sub.get_subject().to_string();
                current_user_id = Some(user_id.clone());
                let mut roles = self
                    .user_manage_svc
                    .list_user_roles_by_user_id(user_id)
                    .await?;
                // enrich basic roles for gateway
                roles.iter_mut().for_each(|(_, roles)| {
                    roles.insert(Role::Anonymous);
                    roles.insert(Role::Untagged);
                });
                roles
                    .entry(Platform::Gateway)
                    .or_insert(HashSet::new())
                    .insert(Role::Untagged);
                roles
            }
        };
        Ok((current_user_id, user_roles))
    }

    pub async fn list_user_privilege(
        &self,
        headers: HeaderMap,
        platform: Option<Platform>,
    ) -> Result<Vec<PrivilegeRule>> {
        let platform = platform.unwrap_or(Platform::Gateway);
        let (_, user_roles) = self.get_user_roles(headers).await?;
        let roles = match user_roles.get(&platform) {
            Some(roles) => roles,
            None => return Ok(vec![]),
        };
        let privileges = self
            .cluster_manage_svc
            .list_all_privilege_configs_of_platform(platform)
            .await
            .into_iter()
            .filter_map(|(_, p)| {
                let backend_apis = p
                    .backend_apis
                    .iter()
                    .filter(|api| api.roles.intersection(roles).count() > 0)
                    .cloned()
                    .collect::<Vec<_>>();
                if backend_apis.is_empty() {
                    return None;
                }
                let rule = PrivilegeRule {
                    config_key: p.config_key.clone(),
                    backend_apis,
                    platform: p.platform.clone(),
                };
                Some(rule)
            })
            .collect::<Vec<_>>();
        Ok(privileges)
    }
}
