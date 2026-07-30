use std::collections::{HashMap, HashSet};

use anyhow::Result;
use http::HeaderMap;

use crate::{
    domain::{
        model::{
            entity::{privilege_rule::PrivilegeRule, user_info::UserSubject},
            value::{platform::Platform, role::Role},
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
        let settings = Settings::get();
        let access_token_name = settings
            .auth_args
            .svc_cookie_access_token_key
            .as_ref()
            .map(|s| s.as_str());
        let refresh_token_name = settings
            .auth_args
            .svc_cookie_refresh_token_key
            .as_ref()
            .map(|s| s.as_str());
        let user_subject =
            UserSubject::from_header(&headers, access_token_name, refresh_token_name)?;

        let user_subject = match user_subject {
            Some(user_subject) => user_subject,
            None => {
                return Ok((None, [(Platform::Gateway, [Role::Anonymous].into())].into()));
            }
        };

        let user_id = user_subject.subject.get_subject_value().to_string();
        let mut roles = self
            .user_manage_svc
            .list_user_roles_by_user_id(user_id.clone())
            .await?;
        // enrich basic roles for gateway
        roles.iter_mut().for_each(|(_, roles)| {
            roles.insert(Role::Anonymous);
            roles.insert(Role::Untagged);
        });
        roles
            .entry(Platform::Gateway)
            .or_insert(HashSet::new())
            .extend([Role::Anonymous, Role::Untagged]);
        Ok((Some(user_id), roles))
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
