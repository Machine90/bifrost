use std::{collections::HashSet, sync::Arc};

use anyhow::Result;

use crate::domain::model::{
    entity::user_config::{UserConfig, UserConfigAdd, UserConfigEdit, UserConfigQuery},
    value::role::Role,
};

#[async_trait::async_trait]
pub trait UserRepository: Send + Sync + 'static {
    /// Preloading context into repository.
    async fn preloading(&self) -> Result<()>;

    async fn get_user_config(&self, user_id: &str) -> Result<Option<Arc<UserConfig>>>;

    async fn insert_user_config(&self, user_config: UserConfigAdd) -> Result<Option<i32>>;

    async fn update_user_config(&self, user_config: UserConfigEdit) -> Result<()>;

    async fn delete_user_config(&self, config_id: i32) -> Result<()>;

    async fn get_distinct_roles(&self) -> HashSet<Role>;

    async fn list_user_configs_by_ids(
        &self,
        user_ids: HashSet<String>,
    ) -> Result<Vec<UserConfigQuery>>;

    async fn rename_role_cache(&self, old_role: Role, new_role: Role);
}
