use std::{sync::Arc, time::Duration, u64};

use moka2::future::Cache;

use crate::domain::model::entity::user_info::UserBaseInfo;

const DEFAULT_MAX_LOGGED_IN_USER_ENTRY_COUNT: u64 = 10_000_000;

#[derive(Clone)]
pub(crate) struct UserLeaseCache {
    user_lease_map: Arc<Cache<String, Arc<()>>>,
}

impl UserLeaseCache {
    pub(crate) fn new(max_capacity: Option<u64>, lease_tti_timeout: Duration) -> Self {
        let cap = max_capacity.unwrap_or(DEFAULT_MAX_LOGGED_IN_USER_ENTRY_COUNT);
        let cache = Cache::builder()
            .max_capacity(cap)
            .time_to_idle(lease_tti_timeout)
            .build();
        Self {
            user_lease_map: Arc::new(cache),
        }
    }

    pub(crate) fn contains(&self, user_info: &UserBaseInfo) -> bool {
        let key = user_info.user_subject.subject.get_suggest_cache_key();
        self.user_lease_map.contains_key(&key)
    }

    pub(crate) async fn add(&self, user_info: &UserBaseInfo) {
        let key = user_info.user_subject.subject.get_suggest_cache_key();
        self.user_lease_map.insert(key, Arc::new(())).await
    }
}
