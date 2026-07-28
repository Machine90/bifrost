use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use anyhow::Context;
use partialdebug::placeholder::PartialDebug;

use crate::domain::{
    model::{
        entity::user_config::{UserConfigAdd, UserConfigEdit, UserConfigQuery},
        value::{platform::Platform, role::Role},
    },
    repository::user_repository::UserRepository,
};

#[derive(PartialDebug, Clone)]
pub struct UserManageService {
    user_repo: Arc<dyn UserRepository>,
}

impl UserManageService {
    pub fn new(user_repo: Arc<dyn UserRepository>) -> Self {
        Self { user_repo }
    }

    pub async fn list_user_roles_by_user_id(
        &self,
        user_id: String,
    ) -> anyhow::Result<HashMap<Platform, HashSet<Role>>> {
        let roles = self
            .user_repo
            .get_user_config(&user_id)
            .await?
            .map(|v| {
                let platform_roles = v.list_platforms().clone();
                platform_roles
            })
            .unwrap_or_default();
        Ok(roles)
    }

    pub async fn add_new_user_config(
        &self,
        user_config: UserConfigAdd,
    ) -> anyhow::Result<Option<i32>> {
        let added_config_id = self
            .user_repo
            .insert_user_config(user_config)
            .await
            .context("Failed to add user config")?;
        Ok(added_config_id)
    }

    pub async fn edit_new_user_config(&self, user_config: UserConfigEdit) -> anyhow::Result<()> {
        self.user_repo
            .update_user_config(user_config)
            .await
            .context("Failed to edit user config")?;
        Ok(())
    }

    pub async fn delete_user_config(&self, config_id: i32) -> anyhow::Result<()> {
        self.user_repo
            .delete_user_config(config_id)
            .await
            .context("Failed to delete user config")?;
        Ok(())
    }

    pub async fn list_user_config_by_user_ids(
        &self,
        user_ids: Vec<String>,
    ) -> anyhow::Result<Vec<UserConfigQuery>> {
        if user_ids.is_empty() {
            return Ok(vec![]);
        }
        let user_configs = self
            .user_repo
            .list_user_configs_by_ids(user_ids.into_iter().collect())
            .await
            .context("Failed to list user configs by user ids")?;
        Ok(user_configs)
    }
}
