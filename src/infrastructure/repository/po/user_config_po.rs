use std::collections::{HashMap, HashSet};

use crate::{
    common::constants::mock::MOCKED_OPERATE_USER_ID,
    domain::model::{
        entity::user_config::{UserConfig, UserConfigAdd, UserConfigEdit},
        value::{platform::Platform, role::Role},
    },
    schemas::bifrost::gateway_user_config,
};
use cookie::time::OffsetDateTime;
use diesel::{AsChangeset, Insertable, Queryable, Selectable};
use partialdebug::placeholder::PartialDebug;

#[derive(PartialDebug, Selectable, Insertable, Queryable)]
#[diesel(table_name = gateway_user_config)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct UserConfigCreatePo {
    pub(crate) user_id: String,
    pub(crate) platform: String,
    pub(crate) roles: Vec<Option<String>>,
    pub(crate) operator_id: String,
}

#[derive(PartialDebug, Selectable, AsChangeset, Queryable)]
#[diesel(table_name = gateway_user_config)]
pub(crate) struct UserConfigUpdatePo {
    pub(crate) user_id: String,
    pub(crate) platform: String,
    pub(crate) roles: Option<Vec<String>>,
    pub(crate) operator_id: String,
}

impl UserConfigUpdatePo {
    pub(crate) fn should_update(&self) -> bool {
        self.roles.is_some()
    }
}

#[derive(PartialDebug, Selectable, Insertable, Queryable)]
#[diesel(table_name = gateway_user_config)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct UserConfigQueryPo {
    pub(crate) id: i32,
    pub(crate) user_id: String,
    pub(crate) platform: String,
    pub(crate) roles: Vec<Option<String>>,
    pub(crate) operator_id: String,
    pub(crate) ctime: Option<OffsetDateTime>,
    pub(crate) mtime: Option<OffsetDateTime>,
}

impl Into<UserConfig> for UserConfigQueryPo {
    fn into(self) -> UserConfig {
        let source = Platform::from_str(&self.platform);
        let roles = self
            .roles
            .iter()
            .filter_map(|role| role.as_ref().map(Role::from_str))
            .collect::<HashSet<_>>();
        let mut platform_to_roles = HashMap::new();
        platform_to_roles.insert(source, roles);
        let one_platform_config = UserConfig::new(platform_to_roles);
        one_platform_config
    }
}

impl From<UserConfigAdd> for UserConfigCreatePo {
    fn from(add: UserConfigAdd) -> Self {
        let UserConfigAdd {
            user_id,
            platform,
            roles,
        } = add;
        Self {
            user_id,
            platform: platform.to_string(),
            roles: roles
                .iter()
                .map(ToString::to_string)
                .map(|r| Some(r))
                .collect(),
            operator_id: MOCKED_OPERATE_USER_ID.to_string(),
        }
    }
}

impl From<UserConfigEdit> for UserConfigUpdatePo {
    fn from(edit: UserConfigEdit) -> Self {
        let UserConfigEdit {
            user_id,
            platform,
            roles,
        } = edit;
        Self {
            user_id,
            platform: platform.to_string(),
            roles: roles.map(|roles| roles.iter().map(ToString::to_string).collect()),
            operator_id: MOCKED_OPERATE_USER_ID.to_string(),
        }
    }
}
