use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Arc,
};

use anyhow::Result;
use http::Method;
use pingora::lb::Backend;

use crate::domain::model::{
    entity::{
        privilege_rule::{PrivilegeRule, PrivilegeRuleEdit},
        rename_role_result::RenameRoleResult,
        route_config::RouteConfig,
        service_backends::ServiceBackends,
    },
    value::{
        platform::Platform,
        role::Role,
        service::{Service, ServiceRegistry},
    },
};

#[async_trait::async_trait]
pub trait ClusterRepository: Send + Sync + 'static {
    async fn preloading(
        &self,
        static_service_privileges: Vec<PrivilegeRule>,
        static_service_backends: HashMap<String, BTreeSet<Backend>>,
    ) -> Result<()>;

    fn list_services(&self) -> HashSet<Service>;

    /// Get all backend endpoints of `service_name` from all kinds of
    /// [ServiceRegistry](bifrost::domain::model::value::service::ServiceRegistry)
    async fn match_service_backends(
        &self,
        service_name: &str,
    ) -> HashMap<ServiceRegistry, ServiceBackends>;

    async fn set_service_backends(
        &self,
        registry: ServiceRegistry,
        service_instances: &HashMap<String, BTreeSet<Backend>>,
    );

    async fn get_route_config(
        &self,
        platform: &Platform,
        service: &str,
        method: &Method,
        path: &str,
        use_match: bool,
    ) -> Result<Option<Arc<RouteConfig>>>;

    async fn get_distinct_roles(&self) -> HashSet<Role>;

    async fn get_distinct_platform(&self) -> HashSet<Platform>;

    /// Insert new record of privilege rule into repository and
    /// returns a corresponding rule id.
    async fn insert_privilege_config(&self, privilege_rule: PrivilegeRule) -> Result<Option<i32>>;

    /// Check if a privilege rule can be edit, then return true.
    async fn is_privilege_editable(&self, rule_id: i32) -> bool;

    async fn update_privilege_config(
        &self,
        rule_id: i32,
        privilege_edit: PrivilegeRuleEdit,
    ) -> Result<()>;

    async fn delete_privilege_config(&self, rule_id: i32) -> Result<()>;

    /// List specified platform ranged privilege configs from repository.
    async fn list_privilege_configs(
        &self,
        platform: Option<Platform>,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Vec<(i32, Arc<PrivilegeRule>)>;

    async fn rename_role(&self, old_role: Role, new_role: Role) -> Result<RenameRoleResult>;
}
