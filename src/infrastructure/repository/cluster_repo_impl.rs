use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Arc,
};

use anyhow::{Context, Result};
use diesel_async::AsyncConnection;
use diesel_json::Json;
use futures::{Stream, StreamExt, stream};
use http::Method;
use pingora::lb::Backend;
use tracing::Level;

use crate::{
    common::{
        constants::{dao, mock::MOCKED_OPERATE_USER_ID},
        sentry_ext::SentryError,
        tracing_ext::{TracingErrorExt, TracingResultExt},
    },
    domain::{
        model::{
            entity::{
                backend_api_rule::BackendApiRule,
                privilege_rule::{PrivilegeRule, PrivilegeRuleEdit},
                rename_role_result::RenameRoleResult,
                route_config::{RouteConfig, RouterPath},
                service_backends::ServiceBackends,
            },
            value::{
                platform::Platform,
                role::Role,
                service::{Service, ServiceRegistry},
            },
        },
        repository::cluster_repository::ClusterRepository,
    },
    infrastructure::{
        repository::{
            cache::{
                privilege_cache::PrivilegeCache, router_cache::RoutersCache,
                service_instance_cache::ServiceInstanceCache,
            },
            dao::{privilege_config_dao, user_config_dao},
            po::privilege_po::{PrivilegeCreatePo, PrivilegeQueryPo, PrivilegeUpdatePo},
        },
        utility::bb8_connection_pool::PgPool,
    },
};

#[derive(Debug, Clone)]
pub(crate) struct ClusterRepositoryImpl {
    pg_pool: Arc<PgPool>,
    router_cache: RoutersCache,
    privilege_cache: PrivilegeCache,
    service_instance_cache: ServiceInstanceCache,
}

#[async_trait::async_trait]
impl ClusterRepository for ClusterRepositoryImpl {
    async fn preloading(
        &self,
        static_service_privileges: Vec<PrivilegeRule>,
        static_service_backends: HashMap<String, BTreeSet<Backend>>,
    ) -> Result<()> {
        // load privilege into memory.
        let mut results = self.list_privilege_configs_dao().await?;
        while let Some(batched_records) = results.next().await {
            let batched_records = batched_records
                .context("Failed to fetch privilege config")
                .log_if_error(Level::ERROR)
                .report_sentry()?;
            for query_po in batched_records {
                self.upsert_privilege_cache(query_po.id, query_po.into())
                    .await?;
            }
        }

        let mut static_rule_id = -1;
        for privilege in static_service_privileges {
            self.upsert_privilege_cache(static_rule_id, privilege)
                .await?;
            static_rule_id -= 1;
        }
        self.set_service_backends(ServiceRegistry::Static, &static_service_backends)
            .await;
        Ok(())
    }

    fn list_services(&self) -> HashSet<Service> {
        self.service_instance_cache.list_services()
    }

    async fn match_service_backends(
        &self,
        service_name: &str,
    ) -> HashMap<ServiceRegistry, ServiceBackends> {
        self.service_instance_cache
            .match_service_backends(service_name)
            .await
    }

    async fn set_service_backends(
        &self,
        registry: ServiceRegistry,
        service_instances: &HashMap<String, BTreeSet<Backend>>,
    ) {
        self.service_instance_cache
            .set_service_backends(registry, service_instances)
            .await;
    }

    async fn get_route_config(
        &self,
        platform: &Platform,
        service: &str,
        method: &Method,
        path: &str,
        use_match: bool,
    ) -> Result<Option<Arc<RouteConfig>>> {
        let config = if use_match {
            let config = self
                .router_cache
                .match_route_config(platform, service, method, path)
                .await;
            config
        } else {
            let router_path = RouterPath {
                method: method.clone(),
                url_path: path.to_string(),
            };
            let config = self
                .router_cache
                .get_route_exactly(platform, service, router_path)
                .await;
            config
        };

        // we don't use database record as fallback when cache is missing in this
        // scenarios, because gateway is a high-concurrency component, if the cache is
        // missing, all concurrency read will hit database, may cause some dangerous
        // result, and there are not a huge number of config saved in the memory.
        // so we should try best to guarantee all configs cached in memory whatever.
        Ok(config)
    }

    async fn is_privilege_editable(&self, rule_id: i32) -> bool {
        self.privilege_cache
            .get_rule(rule_id)
            .await
            .map(|rule| rule.is_editable())
            .unwrap_or(false)
    }

    async fn get_distinct_roles(&self) -> HashSet<Role> {
        self.privilege_cache.distinct_roles()
    }

    async fn get_distinct_platform(&self) -> HashSet<Platform> {
        self.privilege_cache.distinct_platform()
    }

    async fn insert_privilege_config(&self, privilege_rule: PrivilegeRule) -> Result<Option<i32>> {
        let rule_id = self
            .insert_privilege_config_dao(privilege_rule.clone())
            .await
            .context("Failed to insert_privilege_config")
            .log_if_error(Level::ERROR)
            .report_sentry()?;
        if let Some(rule_id) = rule_id {
            if !self.privilege_cache.contains_rule(rule_id) {
                self.upsert_privilege_cache(rule_id, privilege_rule).await?;
            }
        }
        Ok(rule_id)
    }

    async fn update_privilege_config(
        &self,
        rule_id: i32,
        privilege_rule: PrivilegeRuleEdit,
    ) -> Result<()> {
        let changed = self
            .update_privilege_config_dao(rule_id, privilege_rule.clone())
            .await
            .context("Failed to update_privilege_config_dao")
            .report_sentry()?;
        if let Some(rule) = changed {
            // clear cache before update, and then upsert cache with new.
            self.clear_privilege_cache(rule_id).await;
            self.upsert_privilege_cache(rule_id, rule.try_into()?)
                .await?;
        }
        Ok(())
    }

    async fn delete_privilege_config(&self, rule_id: i32) -> Result<()> {
        if let Some(rule) = self
            .delete_privilege_config_dao(rule_id)
            .await
            .context("Failed to delete_privilege_config")
            .log_if_error(Level::ERROR)
            .report_sentry()?
        {
            self.prune_privilege_cache(rule_id, rule.into()).await;
        }
        Ok(())
    }

    async fn list_privilege_configs(
        &self,
        platform: Option<Platform>,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Vec<(i32, Arc<PrivilegeRule>)> {
        self.list_privilege_configs_cache(platform, offset, limit)
            .await
    }

    async fn rename_role(&self, old_role: Role, new_role: Role) -> Result<RenameRoleResult> {
        let renamed = self
            .rename_role_dao(&old_role.to_string(), &new_role.to_string())
            .await?;
        if renamed.updated_privilege_count > 0 {
            self.rename_role_cache(old_role, new_role).await;
        }
        Ok(renamed)
    }
}

impl ClusterRepositoryImpl {
    pub fn new(
        pg_pool: Arc<PgPool>,
        service_instance_cache: ServiceInstanceCache,
        router_cache: RoutersCache,
    ) -> Self {
        Self {
            pg_pool,
            router_cache,
            privilege_cache: PrivilegeCache::new(None),
            service_instance_cache,
        }
    }

    pub(crate) async fn insert_privilege_config_dao(
        &self,
        privilege_rule: PrivilegeRule,
    ) -> Result<Option<i32>> {
        let mut conn = self.pg_pool.get_connection().await?;
        let po = PrivilegeCreatePo::from(privilege_rule);
        let rule_id = privilege_config_dao::insert_record(&mut conn, po).await?;
        Ok(rule_id)
    }

    pub(crate) async fn update_privilege_config_dao(
        &self,
        rule_id: i32,
        privilege_rule: PrivilegeRuleEdit,
    ) -> Result<Option<PrivilegeQueryPo>> {
        let mut conn = self.pg_pool.get_connection().await?;
        let po = PrivilegeUpdatePo {
            id: rule_id,
            platform: privilege_rule.platform.map(|v| v.to_string()),
            config_key: privilege_rule.config_key,
            backend_rules: privilege_rule
                .backend_apis
                .map(|apis| Json(apis.into_iter().map(Into::into).collect::<Vec<_>>())),
            config_version: None,
            operator_id: MOCKED_OPERATE_USER_ID.to_string(),
        };
        let changed_po = privilege_config_dao::update_record(&mut conn, po).await?;
        Ok(changed_po)
    }

    pub(crate) async fn delete_privilege_config_dao(
        &self,
        rule_id: i32,
    ) -> Result<Option<PrivilegeQueryPo>> {
        let mut conn = self.pg_pool.get_connection().await?;
        let removed = privilege_config_dao::delete_record(&mut conn, rule_id).await?;
        Ok(removed)
    }

    pub(crate) async fn list_privilege_configs_dao(
        &self,
    ) -> Result<impl Stream<Item = Result<Vec<PrivilegeQueryPo>>>> {
        let mut conn = self.pg_pool.get_connection().await?;
        let total = privilege_config_dao::fetch_count(&mut conn).await?;
        drop(conn);

        let total_pages = (total / dao::FETCH_PRIVILEGE_PAGE_SIZE) + 1;
        const FETCH_PRIVILEGE_CONCURRENCY: usize = 2;

        let result = stream::iter(1..=total_pages)
            .map(move |page| async move {
                let offset = (page - 1) * dao::FETCH_PRIVILEGE_PAGE_SIZE;
                let mut conn = self.pg_pool.get_connection().await?;
                let records = privilege_config_dao::fetch_any(
                    &mut conn,
                    offset,
                    dao::FETCH_PRIVILEGE_PAGE_SIZE,
                )
                .await;
                records
            })
            .buffer_unordered(FETCH_PRIVILEGE_CONCURRENCY);
        Ok(result)
    }

    pub(crate) async fn rename_role_dao(
        &self,
        old_role: &str,
        new_role: &str,
    ) -> Result<RenameRoleResult> {
        let mut conn = self.pg_pool.get_connection().await?;
        let result = conn
            .transaction(async move |conn| {
                let updated_privilege_count = privilege_config_dao::rename_role(
                    conn,
                    old_role.to_string(),
                    new_role.to_string(),
                )
                .await?;
                let updated_user_count =
                    user_config_dao::rename_role(conn, old_role.to_string(), new_role.to_string())
                        .await?;
                anyhow::Ok(RenameRoleResult {
                    updated_privilege_count,
                    updated_user_count,
                })
            })
            .await?;
        Ok(result)
    }

    pub(crate) async fn upsert_privilege_cache(
        &self,
        rule_id: i32,
        privilege_rule: PrivilegeRule,
    ) -> anyhow::Result<()> {
        for api in privilege_rule.backend_apis.iter() {
            let platform = privilege_rule.platform.clone();
            let mut platform_roles = HashMap::new();
            platform_roles.insert(platform, api.roles.clone());
            let new_config =
                RouteConfig::new_without_config(&api.method, &api.url_path, platform_roles);
            if let Err(e) = self
                .router_cache
                .upsert_route_config(&api.service, new_config, true)
                .await
            {
                e.context(format!(
                    "service = {}, method = {}, url = {}",
                    api.service, api.method, api.url_path
                ))
                .context("Failed to upsert route")
                .log_as_error();
            }
        }
        self.privilege_cache
            .insert_rule(rule_id, privilege_rule)
            .await;
        Ok(())
    }

    pub(crate) async fn prune_privilege_cache(&self, rule_id: i32, privilege_rule: PrivilegeRule) {
        let platform = privilege_rule.platform;
        for BackendApiRule {
            service,
            url_path,
            method,
            ..
        } in privilege_rule.backend_apis
        {
            if let Err(e) = self
                .router_cache
                .prune_platform_route_config(&platform, &service, &method, &url_path)
                .await
            {
                tracing::warn!(error = ?e, "Failed to prune route config");
            }
        }
        self.privilege_cache.prune(rule_id).await;
    }

    pub(crate) async fn clear_privilege_cache(&self, rule_id: i32) -> Option<()> {
        let rule = self.privilege_cache.get_rule(rule_id).await?;
        self.prune_privilege_cache(rule_id, rule.as_ref().clone())
            .await;
        Some(())
    }

    pub(crate) async fn list_privilege_configs_cache(
        &self,
        platform: Option<Platform>,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Vec<(i32, Arc<PrivilegeRule>)> {
        let total_entries_count = self.privilege_cache.size().await;
        let offset = offset.unwrap_or(0);
        let total_remained = total_entries_count - offset;
        let limit = limit.unwrap_or(total_remained).min(total_remained);
        let result = self
            .privilege_cache
            .list(&platform)
            .await
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect::<Vec<_>>();
        result
    }

    pub(crate) async fn rename_role_cache(&self, old_role: Role, new_role: Role) {
        let changes = self.privilege_cache.rename_role(&old_role, &new_role).await;
        for (_, privilege_rule) in changes {
            for api in privilege_rule.backend_apis.iter() {
                let platform = privilege_rule.platform.clone();
                let mut platform_roles = HashMap::new();
                platform_roles.insert(platform, api.roles.clone());
                let new_config =
                    RouteConfig::new_without_config(&api.method, &api.url_path, platform_roles);
                if let Err(e) = self
                    .router_cache
                    .upsert_route_config(&api.service, new_config, true)
                    .await
                {
                    e.context(format!(
                        "service = {}, method = {}, url = {}",
                        api.service, api.method, api.url_path
                    ))
                    .context("Failed to upsert route")
                    .log_as_error();
                }
            }
        }
    }
}
