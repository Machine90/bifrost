use std::{collections::HashSet, str::FromStr};

use crate::{
    common::{
        constants::mock::{MOCKED_CONFIG_VERSION, MOCKED_OPERATE_USER_ID},
        tracing_ext::TracingResultExt,
    },
    domain::model::{
        entity::{backend_api_rule::BackendApiRule, privilege_rule::PrivilegeRule},
        value::{platform::Platform, role::Role},
    },
    schemas::bifrost::gateway_privilege_config,
};
use anyhow::Context;
use cookie::time::OffsetDateTime;
use diesel::{AsChangeset, Insertable, Queryable, Selectable};
use diesel_json::Json;
use http::Method;
use partialdebug::placeholder::PartialDebug;
use serde::{Deserialize, Serialize};
use tracing::Level;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct BackendRulePo {
    pub(crate) key: Option<String>,
    pub(crate) service: String,
    pub(crate) method: String,
    pub(crate) url_path: String,
    pub(crate) roles: HashSet<String>,
}

#[derive(PartialDebug, Selectable, Insertable, Queryable)]
#[diesel(table_name = gateway_privilege_config)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct PrivilegeCreatePo {
    pub(crate) platform: String,
    pub(crate) config_key: String,
    pub(crate) backend_rules: Json<Vec<BackendRulePo>>,
    pub(crate) operator_id: String,
    pub(crate) config_version: i32,
}

#[derive(PartialDebug, Selectable, AsChangeset, Queryable)]
#[diesel(table_name = gateway_privilege_config)]
pub(crate) struct PrivilegeUpdatePo {
    pub(crate) id: i32,
    pub(crate) platform: Option<String>,
    pub(crate) config_key: Option<String>,
    pub(crate) backend_rules: Option<Json<Vec<BackendRulePo>>>,
    pub(crate) config_version: Option<i32>,
    pub(crate) operator_id: String,
}

impl PrivilegeUpdatePo {
    pub(crate) fn should_update(&self) -> bool {
        self.platform.is_some()
            || self.config_key.is_some()
            || self.backend_rules.is_some()
            || self.config_version.is_some()
    }
}

#[derive(PartialDebug, Selectable, Insertable, Queryable)]
#[diesel(table_name = gateway_privilege_config)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct PrivilegeQueryPo {
    pub(crate) id: i32,
    pub(crate) platform: String,
    pub(crate) config_key: String,
    pub(crate) backend_rules: Json<Vec<BackendRulePo>>,
    pub(crate) operator_id: String,
    pub(crate) config_version: i32,
    pub(crate) ctime: Option<OffsetDateTime>,
    pub(crate) mtime: Option<OffsetDateTime>,
}

impl From<BackendApiRule> for BackendRulePo {
    fn from(rule: BackendApiRule) -> Self {
        let BackendApiRule {
            key,
            service,
            url_path,
            roles,
            method,
        } = rule;
        Self {
            key,
            service,
            url_path,
            roles: roles.into_iter().map(|r| r.to_string()).collect(),
            method: method.as_str().to_string(),
        }
    }
}

impl TryInto<BackendApiRule> for BackendRulePo {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<BackendApiRule, Self::Error> {
        let Self {
            key,
            service,
            url_path,
            roles,
            method,
        } = self;
        Ok(BackendApiRule {
            method: Method::from_str(&method.to_uppercase())
                .context(format!("key = {key:?}"))
                .context("Failed to convert rule method")?,
            key,
            service,
            url_path,
            roles: roles.iter().map(Role::from_str).collect(),
        })
    }
}

impl From<PrivilegeRule> for PrivilegeCreatePo {
    fn from(rule: PrivilegeRule) -> Self {
        let PrivilegeRule {
            config_key,
            backend_apis,
            platform,
        } = rule;
        let backend_rules = backend_apis
            .into_iter()
            .map(BackendRulePo::from)
            .collect::<Vec<_>>();
        Self {
            platform: platform.to_string(),
            config_key,
            backend_rules: Json(backend_rules),
            operator_id: MOCKED_OPERATE_USER_ID.to_string(),
            config_version: MOCKED_CONFIG_VERSION,
        }
    }
}

impl Into<PrivilegeRule> for PrivilegeQueryPo {
    fn into(self) -> PrivilegeRule {
        let Self {
            platform,
            config_key,
            backend_rules,
            ..
        } = self;
        let backend_apis = backend_rules
            .0
            .into_iter()
            .filter_map(|po| {
                let api = po
                    .try_into()
                    .context("Failed to convert api rule")
                    .log_if_error(Level::ERROR)
                    .ok()?;
                Some(api)
            })
            .collect();
        let rule = PrivilegeRule {
            config_key,
            backend_apis,
            platform: Platform::from_str(platform.as_str()),
        };
        rule
    }
}
