use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use anyhow::{Context, Result};
use futures::{Stream, StreamExt, stream};
use partialdebug::placeholder::PartialDebug;
use tracing::Level;

use crate::{
    common::{constants::dao, sentry_ext::SentryError, tracing_ext::TracingResultExt},
    domain::{
        model::{
            entity::user_config::{UserConfig, UserConfigAdd, UserConfigEdit, UserConfigQuery},
            value::{platform::Platform, role::Role},
        },
        repository::user_repository::UserRepository,
    },
    infrastructure::{
        repository::{
            cache::user_config_cache::UserConfigCache,
            dao::user_config_dao,
            po::user_config_po::{UserConfigCreatePo, UserConfigQueryPo, UserConfigUpdatePo},
        },
        utility::bb8_connection_pool::PgPool,
    },
};

#[derive(PartialDebug, Clone)]
pub struct UserRepositoryImpl {
    pg_pool: Arc<PgPool>,
    user_cache: UserConfigCache,
}

impl UserRepositoryImpl {
    pub async fn new(pg_pool: Arc<PgPool>, initial_gateway_admin_ids: Vec<String>) -> Self {
        let user_cache = UserConfigCache::new(None);
        for gateway_admin_user_id in initial_gateway_admin_ids {
            let mut default_roles = HashMap::new();
            default_roles.insert(Platform::Gateway, [Role::GatewayAdmin].into());
            let config = UserConfig::new(default_roles);
            user_cache
                .insert_user_config(gateway_admin_user_id, config)
                .await;
        }
        Self {
            pg_pool,
            user_cache,
        }
    }
}

#[async_trait::async_trait]
impl UserRepository for UserRepositoryImpl {
    /// Preloading context into repository.
    async fn preloading(&self) -> Result<()> {
        let mut user_config = self.list_user_configs_dao(Some(1_000)).await?;
        while let Some(Ok(batch)) = user_config.next().await {
            for po in batch {
                let user_id = po.user_id.clone();
                self.user_cache.upsert_user_config(user_id, po.into()).await;
            }
        }
        Ok(())
    }

    async fn get_user_config(&self, user_id: &str) -> Result<Option<Arc<UserConfig>>> {
        let mut user_config = self.user_cache.get_user_config(user_id).await;
        if let Some(user_config) = user_config {
            return Ok(Some(user_config));
        }
        let persisted_config = self
            .get_user_config_dao(user_id)
            .await
            .context("Failed to get_user_config_dao")
            .report_sentry()
            .log_if_error(Level::ERROR)?;
        if let Some(config) = persisted_config {
            let cached_value = self
                .user_cache
                .insert_user_config(user_id.to_string(), config)
                .await;
            user_config = Some(cached_value);
        }
        Ok(user_config)
    }

    async fn insert_user_config(&self, user_config: UserConfigAdd) -> Result<Option<i32>> {
        let po = self
            .insert_user_config_dao(user_config.clone())
            .await
            .context("Failed to insert_user_config_dao")
            .report_sentry()
            .log_if_error(Level::ERROR)?;
        let mut config_id = None;
        if let Some(po) = po {
            config_id = Some(po.id);
            self.upsert_user_config_cache(po).await;
        }
        Ok(config_id)
    }

    async fn update_user_config(&self, user_config: UserConfigEdit) -> Result<()> {
        let changed = self
            .update_user_config_dao(user_config.clone())
            .await
            .context("Failed to update_user_config")
            .report_sentry()
            .log_if_error(Level::ERROR)?;
        if let Some(config) = changed {
            self.upsert_user_config_cache(config).await;
        }
        Ok(())
    }

    async fn delete_user_config(&self, config_id: i32) -> Result<()> {
        if let Some(config) = self
            .delete_user_config_dao(config_id)
            .await
            .context("Failed to delete_user_config_dao")
            .report_sentry()
            .log_if_error(Level::ERROR)?
        {
            let platform = Platform::from_str(&config.platform);
            self.prune_user_config_cache(platform, config.user_id).await;
        }
        Ok(())
    }

    async fn get_distinct_roles(&self) -> HashSet<Role> {
        self.user_cache.distinct_roles().await
    }

    async fn list_user_configs_by_ids(
        &self,
        user_ids: HashSet<String>,
    ) -> Result<Vec<UserConfigQuery>> {
        let configs = self
            .list_user_configs_by_ids_dao(user_ids)
            .await
            .context("Failed to list_user_configs_by_ids_dao")
            .report_sentry()
            .log_if_error(Level::ERROR)?
            .into_iter()
            .map(|po| UserConfigQuery {
                config_id: po.id,
                user_id: po.user_id,
                platform: Platform::from_str(&po.platform),
                roles: po
                    .roles
                    .into_iter()
                    .filter_map(|role| role)
                    .map(|r| Role::from_str(&r))
                    .collect(),
            })
            .collect::<Vec<_>>();
        Ok(configs)
    }

    async fn rename_role_cache(&self, old_role: Role, new_role: Role) {
        self.user_cache.rename_role(old_role, new_role).await;
    }
}

fn fold_user_configs(mut config: UserConfig, record: UserConfigQueryPo) -> UserConfig {
    let one_platform_config = record.into();
    config.merge(&one_platform_config, false);
    config
}

impl UserRepositoryImpl {
    async fn get_user_config_dao(&self, user_id: &str) -> Result<Option<UserConfig>> {
        let mut conn = self.pg_pool.get_connection().await?;
        let records = user_config_dao::fetch_by_user_id(&mut conn, user_id.to_string()).await?;
        if records.is_empty() {
            return Ok(None);
        }
        let user_config = records
            .into_iter()
            .fold(UserConfig::new(HashMap::new()), fold_user_configs);
        Ok(Some(user_config))
    }

    async fn insert_user_config_dao(
        &self,
        user_config: UserConfigAdd,
    ) -> Result<Option<UserConfigQueryPo>> {
        let mut conn = self.pg_pool.get_connection().await?;
        let po = UserConfigCreatePo::from(user_config);
        let po = user_config_dao::insert_record(&mut conn, po).await?;
        return Ok(po);
    }

    pub(crate) async fn update_user_config_dao(
        &self,
        user_config: UserConfigEdit,
    ) -> Result<Option<UserConfigQueryPo>> {
        let mut conn = self.pg_pool.get_connection().await?;
        let po = UserConfigUpdatePo::from(user_config);
        let changed_po = user_config_dao::update_record(&mut conn, po).await?;
        Ok(changed_po)
    }

    pub(crate) async fn delete_user_config_dao(
        &self,
        config_id: i32,
    ) -> Result<Option<UserConfigQueryPo>> {
        let mut conn = self.pg_pool.get_connection().await?;
        let removed = user_config_dao::delete_record(&mut conn, config_id).await?;
        Ok(removed)
    }

    async fn upsert_user_config_cache(&self, user_config: UserConfigQueryPo) {
        let UserConfigQueryPo {
            user_id,
            platform,
            roles,
            ..
        } = user_config;
        let platform = Platform::from_str(&platform);
        let roles = roles
            .into_iter()
            .filter_map(|role| role)
            .map(|r| Role::from_str(&r))
            .collect();
        let new_user_config = UserConfig::new(HashMap::from_iter([(platform, roles)]));
        self.user_cache
            .upsert_user_config(user_id, new_user_config)
            .await;
    }

    async fn prune_user_config_cache(&self, platform: Platform, user_id: String) {
        self.user_cache
            .prune_platform_user_config(&platform, &user_id)
            .await;
    }

    async fn list_user_configs_dao(
        &self,
        limit: Option<usize>,
    ) -> Result<impl Stream<Item = Result<Vec<UserConfigQueryPo>>>> {
        let mut conn = self.pg_pool.get_connection().await?;
        let total = user_config_dao::fetch_count(&mut conn).await?;
        drop(conn);

        let fetch_record_count = limit.unwrap_or(total).min(total);
        let fetch_pages = (fetch_record_count / dao::FETCH_USER_PAGE_SIZE) + 1;
        const FETCH_PRIVILEGE_CONCURRENCY: usize = 2;

        let result = stream::iter(1..=fetch_pages)
            .map(move |page| async move {
                let offset = (page - 1) * dao::FETCH_USER_PAGE_SIZE;
                let mut conn = self.pg_pool.get_connection().await?;
                let records =
                    user_config_dao::fetch_any(&mut conn, offset, dao::FETCH_PRIVILEGE_PAGE_SIZE)
                        .await;
                records
            })
            .buffer_unordered(FETCH_PRIVILEGE_CONCURRENCY);
        Ok(result)
    }

    async fn list_user_configs_by_ids_dao(
        &self,
        user_ids: HashSet<String>,
    ) -> Result<Vec<UserConfigQueryPo>> {
        let mut conn = self.pg_pool.get_connection().await?;
        let records =
            user_config_dao::fetch_by_ids(&mut conn, user_ids.into_iter().collect()).await?;
        Ok(records)
    }
}
