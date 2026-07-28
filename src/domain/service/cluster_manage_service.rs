use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Arc,
};

use anyhow::{Context, anyhow};
use partialdebug::placeholder::PartialDebug;
use pingora::lb::Backend;
use tracing::Level;

use crate::{
    common::{error_types::ErrorKind, tracing_ext::TracingResultExt},
    domain::{
        model::{
            entity::{
                privilege_rule::{PrivilegeRule, PrivilegeRuleEdit},
                rename_role_result::RenameRoleResult,
            },
            value::{
                platform::Platform,
                role::Role,
                service::{Service, ServiceRegistry},
            },
        },
        repository::{cluster_repository::ClusterRepository, user_repository::UserRepository},
    },
};

#[derive(PartialDebug, Clone)]
pub struct ClusterManageService {
    cluster_repo: Arc<dyn ClusterRepository>,
    user_repo: Arc<dyn UserRepository>,
}

impl ClusterManageService {
    pub fn new(
        cluster_repo: Arc<dyn ClusterRepository>,
        user_repo: Arc<dyn UserRepository>,
    ) -> Self {
        Self {
            cluster_repo,
            user_repo,
        }
    }

    pub async fn update_runtime_service_backends(
        &self,
        registry: ServiceRegistry,
        service_instances: &HashMap<String, BTreeSet<Backend>>,
    ) {
        self.cluster_repo
            .set_service_backends(registry, service_instances)
            .await;
    }

    pub fn list_online_services(&self) -> HashSet<Service> {
        self.cluster_repo.list_services()
    }

    pub async fn get_distinct_roles(&self) -> HashSet<Role> {
        let mut user_roles = self.user_repo.get_distinct_roles().await;
        let route_allowed_roles = self.cluster_repo.get_distinct_roles().await;
        user_roles.extend(route_allowed_roles);
        user_roles
    }

    pub async fn get_distinct_platform(&self) -> HashSet<Platform> {
        let platforms = self.cluster_repo.get_distinct_platform().await;
        platforms
    }

    /// Add a privilege rule to repository, if duplicated with config_key and platform,
    /// this rule will be ignored, if you want to update privilege, please call edit method.
    /// If its the first time insert the config, a rule_id will be returned.
    pub async fn add_privilege_config(
        &self,
        privilege: PrivilegeRule,
        check_service_available: bool,
    ) -> anyhow::Result<Option<i32>> {
        // check service in list
        let service_names = self
            .list_online_services()
            .into_iter()
            .map(|s| s.get_name().to_string())
            .collect::<HashSet<_>>();
        let platform = privilege.platform.clone();
        for rule in privilege.backend_apis.iter() {
            if check_service_available && !service_names.contains(&rule.service) {
                return Err(anyhow!("Invalid service"))
                    .context(ErrorKind::BadInput)
                    .context("Invalid service name")?;
            }

            // check if rule of service for platform is already exists.
            let url_path = &rule.url_path;
            let method = &rule.method;
            let service = &rule.service;
            let ori_rule = self
                .cluster_repo
                .get_route_config(&platform, service, method, url_path, false)
                .await?;
            if let Some(_ori_rule) = ori_rule {
                return Err(anyhow!("Rule conflict"))
                    .context(ErrorKind::Conflict)
                    .context(format!(
                        "Rule already exists, method = {method}, url = {url_path}, service = {service}"
                    ))?;
            }
        }
        let rule_id = self
            .cluster_repo
            .insert_privilege_config(privilege)
            .await
            .context("Failed to add privilege rule")
            .log_if_error(Level::ERROR)?;
        Ok(rule_id)
    }

    pub async fn edit_privilege_config(
        &self,
        rule_id: i32,
        privilege_edit: PrivilegeRuleEdit,
        check_service_available: bool,
    ) -> anyhow::Result<()> {
        if !privilege_edit.has_changes() {
            return Ok(());
        }

        if !self.cluster_repo.is_privilege_editable(rule_id).await {
            return Err(anyhow!("Invalid service"))
                .context(format!("rule_id = {rule_id}"))
                .context(ErrorKind::Forbidden)
                .context("Privilege not allowed to edit")?;
        }

        let service_names = self
            .list_online_services()
            .into_iter()
            .map(|s| s.get_name().to_string())
            .collect::<HashSet<_>>();
        if let Some(rules) = privilege_edit.backend_apis.as_ref() {
            for rule in rules.iter() {
                if check_service_available && !service_names.contains(&rule.service) {
                    // return Err(anyhow!("Invalid service"))?;
                    return Err(anyhow!("Invalid service"))
                        .context(ErrorKind::BadInput)
                        .context("Invalid service name")?;
                }
            }
        }

        self.cluster_repo
            .update_privilege_config(rule_id, privilege_edit)
            .await
            .context("Failed to edit privilege rule")
            .log_if_error(Level::ERROR)?;
        Ok(())
    }

    pub async fn list_all_privilege_configs_of_platform(
        &self,
        platform: Platform,
    ) -> Vec<(i32, Arc<PrivilegeRule>)> {
        self.cluster_repo
            .list_privilege_configs(Some(platform), None, None)
            .await
    }

    pub async fn delete_privilege_config(&self, rule_id: i32) -> anyhow::Result<()> {
        if !self.cluster_repo.is_privilege_editable(rule_id).await {
            return Err(anyhow!("Invalid service"))
                .context(format!("rule_id = {rule_id}"))
                .context(ErrorKind::Forbidden)
                .context("Privilege not allowed to edit")?;
        }
        self.cluster_repo
            .delete_privilege_config(rule_id)
            .await
            .context("Failed to delete privilege rule")
            .log_if_error(Level::ERROR)?;
        Ok(())
    }

    pub async fn rename_role(
        &self,
        old_role: &str,
        new_role: &str,
    ) -> anyhow::Result<RenameRoleResult> {
        let old_role = Role::from_str(&old_role.to_string());
        let new_role = Role::from_str(&new_role.to_string());
        match (&old_role, &new_role) {
            (Role::Tagged(_), Role::Tagged(_)) => (),
            _ => {
                // system roles not allowed to change
                return Err(anyhow!("Invalid change target roles")).context(ErrorKind::BadInput)?;
            }
        };
        let result = self
            .cluster_repo
            .rename_role(old_role.clone(), new_role.clone())
            .await
            .context("Failed to rename role")
            .log_if_error(Level::ERROR)?;
        if result.updated_user_count > 0 {
            self.user_repo.rename_role_cache(old_role, new_role).await;
        }
        Ok(result)
    }
}
