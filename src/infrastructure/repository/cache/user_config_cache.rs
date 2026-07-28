use std::{collections::HashSet, fmt::Debug, sync::Arc, u64};

use moka2::future::Cache;

use crate::domain::model::{
    entity::user_config::UserConfig,
    value::{platform::Platform, role::Role},
};

const DEFAULT_MAX_USER_CACHE_ENTRY_COUNT: u64 = 1_000_000;

#[derive(Clone)]
pub(crate) struct UserConfigCache {
    user_config_map: Arc<Cache<String, Arc<UserConfig>>>,
}

impl Debug for UserConfigCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserConfigCache")
            .field(
                "user_config_weighted_size",
                &self.user_config_map.weighted_size(),
            )
            .finish()
    }
}

impl UserConfigCache {
    pub(crate) fn new(max_capacity: Option<u64>) -> Self {
        let cap = max_capacity.unwrap_or(DEFAULT_MAX_USER_CACHE_ENTRY_COUNT);
        Self {
            user_config_map: Arc::new(Cache::new(cap)),
        }
    }

    pub(crate) async fn get_user_config(&self, user_id: &str) -> Option<Arc<UserConfig>> {
        self.user_config_map.get(user_id).await
    }

    pub(crate) async fn insert_user_config(
        &self,
        user_id: String,
        config: UserConfig,
    ) -> Arc<UserConfig> {
        let value = Arc::new(config);
        self.user_config_map.insert(user_id, value.clone()).await;
        value
    }

    pub(crate) async fn upsert_user_config(
        &self,
        user_id: String,
        mut new_config: UserConfig,
    ) -> Arc<UserConfig> {
        let ori_config = self.get_user_config(&user_id).await;
        if let Some(ori_config) = ori_config {
            let mut ori_config = ori_config.as_ref().clone();
            ori_config.merge(&new_config, true);
            new_config = ori_config;
        }
        self.insert_user_config(user_id, new_config).await
    }

    pub(crate) async fn clear_user_config(&self, user_id: &str) {
        self.user_config_map.remove(user_id).await;
    }

    pub(crate) async fn prune_platform_user_config(&self, platform: &Platform, user_id: &str) {
        let ori_config = self.get_user_config(&user_id).await;
        if let Some(ori_config) = ori_config {
            let mut new_config = ori_config.as_ref().clone();
            new_config.remove_platform(platform);
            if new_config.is_empty() {
                self.clear_user_config(user_id).await;
            } else {
                self.insert_user_config(user_id.to_string(), new_config)
                    .await;
            }
        }
    }

    pub(crate) async fn distinct_roles(&self) -> HashSet<Role> {
        self.user_config_map
            .iter()
            .map(|(_, value)| value.distinct_roles())
            .flatten()
            .collect()
    }

    pub(crate) async fn rename_role(&self, old_role: Role, new_role: Role) {
        let mut users_to_update = Vec::new();
        for (user_id, config) in self.user_config_map.iter() {
            if config.distinct_roles().contains(&old_role) {
                let mut updated_config = (*config).clone();
                for (_, roles) in updated_config.roles.iter_mut() {
                    if roles.remove(&old_role) {
                        roles.insert(new_role.clone());
                    }
                }
                users_to_update.push((user_id.to_string(), updated_config));
            }
        }
        for (user_id, updated_config) in users_to_update {
            self.user_config_map
                .insert(user_id, Arc::new(updated_config))
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use anyhow::{Context, Result};
    use rstest::rstest;

    use crate::{
        domain::model::{
            entity::user_config::UserConfig,
            value::{platform::Platform, role::Role},
        },
        infrastructure::repository::cache::user_config_cache::UserConfigCache,
    };

    #[rstest]
    #[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 1))]
    async fn test_user_cache() -> Result<()> {
        let cache = UserConfigCache::new(10.into());
        let _ = cache
            .insert_user_config(
                format!("1"),
                UserConfig::new([(Platform::Gateway, [Role::GatewayAdmin].into())].into()),
            )
            .await;
        // repeated insert
        let _ = cache
            .insert_user_config(
                format!("1"),
                UserConfig::new([(Platform::Gateway, [Role::GatewayAdmin].into())].into()),
            )
            .await;
        let user = cache
            .get_user_config("1")
            .await
            .context("User is missing")?;
        let roles = user.roles(&Platform::Gateway).unwrap_or_default();
        assert_eq!(roles, [Role::GatewayAdmin].into());

        // don't overwrite Gateway admin.
        let sales = Role::Tagged("Sales".to_string());
        let _ = cache
            .upsert_user_config(
                format!("1"),
                UserConfig::new([(Platform::Gateway, [sales.clone()].into())].into()),
            )
            .await;
        let user = cache
            .get_user_config("1")
            .await
            .context("User is missing")?;
        let roles = user.roles(&Platform::Gateway).unwrap_or_default();
        assert_eq!(roles, [Role::GatewayAdmin, sales.clone()].into());

        // overwrite user roles by updating
        let hr = Role::Tagged("Hr".to_string());
        let _ = cache
            .upsert_user_config(
                format!("1"),
                UserConfig::new([(Platform::Gateway, [hr.clone()].into())].into()),
            )
            .await;
        let user = cache
            .get_user_config("1")
            .await
            .context("User is missing")?;
        let roles = user.roles(&Platform::Gateway).unwrap_or_default();
        assert_eq!(roles, [Role::GatewayAdmin, hr.clone()].into());

        // more platform, and gateway admin should always be gateway admin
        let new_platform = Platform::Platform(format!("Enterprise A"));
        let _ = cache
            .upsert_user_config(
                format!("1"),
                UserConfig::new([(new_platform.clone(), [sales.clone()].into())].into()),
            )
            .await;
        let user = cache
            .get_user_config("1")
            .await
            .context("User is missing")?;
        let roles = user.roles(&new_platform).unwrap_or_default();
        assert_eq!(roles, [Role::GatewayAdmin, sales.clone()].into());

        // remove user platform
        cache.prune_platform_user_config(&new_platform, "1").await;
        let user = cache
            .get_user_config("1")
            .await
            .context("User is missing")?;
        let platform = user
            .list_platforms()
            .keys()
            .cloned()
            .collect::<HashSet<_>>();
        assert_eq!(platform, [Platform::Gateway].into());

        let roles = user.distinct_roles();
        assert_eq!(roles, [Role::GatewayAdmin, hr.clone()].into());

        Ok(())
    }
}
