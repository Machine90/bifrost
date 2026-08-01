use std::{collections::HashSet, fmt::Debug, sync::Arc};

use moka2::future::Cache;

use crate::domain::model::{
    entity::privilege_rule::PrivilegeRule,
    value::{platform::Platform, role::Role},
};

#[derive(Clone)]
pub(crate) struct PrivilegeCache {
    cache: Arc<Cache<i32, Arc<PrivilegeRule>>>,
}

impl Debug for PrivilegeCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrivilegeCache")
            .field("cache_weighted_size", &self.cache.weighted_size())
            .finish()
    }
}

impl PrivilegeCache {
    pub(crate) fn new() -> Self {
        Self {
            cache: Arc::new(Cache::builder().build()),
        }
    }

    pub(crate) fn distinct_roles(&self) -> HashSet<Role> {
        let roles = self
            .cache
            .iter()
            .map(|(_, rule)| rule.allowed_roles())
            .flatten()
            .collect::<HashSet<_>>();
        roles
    }

    pub(crate) fn distinct_platform(&self) -> HashSet<Platform> {
        let platforms = self
            .cache
            .iter()
            .map(|(_, rule)| rule.platform.clone())
            .collect::<HashSet<_>>();
        platforms
    }

    pub(crate) async fn get_rule(&self, rule_id: i32) -> Option<Arc<PrivilegeRule>> {
        self.cache.get(&rule_id).await
    }

    pub(crate) fn contains_rule(&self, rule_id: i32) -> bool {
        self.cache.contains_key(&rule_id)
    }

    pub(crate) async fn insert_rule(&self, rule_id: i32, rule: PrivilegeRule) {
        self.cache.insert(rule_id, Arc::new(rule)).await;
    }

    pub(crate) async fn list(
        &self,
        platform: &Option<Platform>,
    ) -> impl IntoIterator<Item = (i32, Arc<PrivilegeRule>)> {
        self.cache
            .iter()
            .filter(|(_, p)| {
                if platform.as_ref().is_none() {
                    return true;
                }
                let current_platform = Some(p.platform.as_str());
                platform.as_ref().map(|v| v.as_str()) == current_platform
            })
            .map(|(k, v)| (*k, v.clone()))
    }

    pub(crate) async fn prune(&self, rule_id: i32) -> bool {
        self.cache.remove(&rule_id).await.is_some()
    }

    pub(crate) async fn size(&self) -> usize {
        self.cache.run_pending_tasks().await;
        self.cache.entry_count() as usize
    }

    pub(crate) async fn rename_role(
        &self,
        old_role: &Role,
        new_role: &Role,
    ) -> Vec<(i32, Arc<PrivilegeRule>)> {
        let mut rules_to_update = Vec::new();

        for (rule_id, rule) in self.cache.iter() {
            if !rule.is_editable() {
                continue;
            }
            if rule.allowed_roles().contains(old_role) {
                let mut updated_rule = (*rule).clone();
                for api_rule in updated_rule.backend_apis.iter_mut() {
                    if api_rule.roles.remove(old_role) {
                        api_rule.roles.insert(new_role.clone());
                    }
                }
                rules_to_update.push((*rule_id.as_ref(), updated_rule));
            }
        }

        let mut changes = vec![];
        for (rule_id, updated_rule) in rules_to_update {
            let rule = Arc::new(updated_rule);
            changes.push((rule_id, rule.clone()));
            self.cache.insert(rule_id, rule).await;
        }
        changes
    }
}
