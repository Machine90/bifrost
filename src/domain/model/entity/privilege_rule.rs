use std::{collections::HashSet, hash::Hash};

use crate::domain::model::{
    entity::backend_api_rule::BackendApiRule,
    value::{platform::Platform, role::Role},
};

#[derive(Debug, Clone)]
pub struct PrivilegeRule {
    pub config_key: String,
    pub backend_apis: Vec<BackendApiRule>,
    pub platform: Platform,
}

#[derive(Debug, Clone)]
pub struct PrivilegeRuleEdit {
    pub config_key: Option<String>,
    pub backend_apis: Option<Vec<BackendApiRule>>,
    pub platform: Option<Platform>,
}

impl PrivilegeRuleEdit {
    pub fn has_changes(&self) -> bool {
        self.config_key.is_some() || self.backend_apis.is_some() || self.platform.is_some()
    }
}

impl PartialEq for PrivilegeRule {
    fn eq(&self, other: &Self) -> bool {
        self.config_key == other.config_key && self.platform == other.platform
    }
}

impl Eq for PrivilegeRule {}

impl Hash for PrivilegeRule {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.config_key.hash(state);
        self.platform.hash(state);
    }
}

impl PrivilegeRule {
    pub fn allowed_roles(&self) -> HashSet<Role> {
        self.backend_apis
            .iter()
            .map(|api| api.roles.clone())
            .flatten()
            .collect()
    }

    pub fn is_editable(&self) -> bool {
        self.platform != Platform::Gateway
    }
}
