use std::collections::{HashMap, HashSet};

use crate::domain::model::value::{platform::Platform, role::Role};

#[derive(Debug, Clone)]
pub struct UserConfig {
    pub roles: HashMap<Platform, HashSet<Role>>,
}

#[derive(Debug, Clone)]
pub struct UserConfigAdd {
    pub user_id: String,
    pub platform: Platform,
    pub roles: HashSet<Role>,
}

#[derive(Debug, Clone)]
pub struct UserConfigEdit {
    pub user_id: String,
    pub platform: Platform,
    pub roles: Option<HashSet<Role>>,
}

#[derive(Debug, Clone)]
pub struct UserConfigQuery {
    pub config_id: i32,
    pub user_id: String,
    pub platform: Platform,
    pub roles: HashSet<Role>,
}

impl UserConfig {
    pub fn new(roles: HashMap<Platform, HashSet<Role>>) -> Self {
        Self { roles }
    }

    pub fn merge(&mut self, other: &Self, overwrite: bool) {
        let other_roles = other.roles.clone();
        for (source, roles) in other_roles {
            self.roles
                .entry(source)
                .and_modify(|my_roles| {
                    let other_roles = roles.clone();
                    if !overwrite {
                        my_roles.extend(other_roles)
                    } else {
                        let is_admin = my_roles.contains(&Role::GatewayAdmin);
                        *my_roles = other_roles;
                        if is_admin {
                            // default gateway admin is always admin
                            my_roles.insert(Role::GatewayAdmin);
                        }
                    }
                })
                .or_insert(roles);
        }
    }

    pub fn remove_platform(&mut self, platform: &Platform) {
        self.roles.remove(&platform);
    }

    pub fn is_empty(&self) -> bool {
        self.roles.is_empty()
    }

    pub fn list_platforms(&self) -> &HashMap<Platform, HashSet<Role>> {
        &self.roles
    }

    pub fn roles(&self, source: &Platform) -> Option<HashSet<Role>> {
        let is_gateway_admin = self
            .roles
            .get(&Platform::Gateway)
            .map(|roles| roles.contains(&Role::GatewayAdmin))
            .unwrap_or(false);
        let mut roles = self.roles.get(source).cloned();
        if is_gateway_admin {
            // gateway admin should be available for every platform.
            roles.as_mut().map(|roles| roles.insert(Role::GatewayAdmin));
        }
        roles
    }

    pub fn distinct_roles(&self) -> HashSet<Role> {
        self.roles
            .iter()
            .map(|(_, roles)| roles.iter())
            .flatten()
            .collect::<HashSet<_>>()
            .into_iter()
            .map(Clone::clone)
            .collect()
    }
}
