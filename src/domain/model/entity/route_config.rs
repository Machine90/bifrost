use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    time::Duration,
};

use http::Method;
use partialdebug::placeholder::PartialDebug;
use pingora_limits::rate::Rate;

use crate::domain::model::value::{platform::Platform, role::Role};

pub struct RateLimiter {
    rate: Rate,
    max_requests_per_sec: isize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UrlKind {
    Absolutely,
    MatchPath,
    MatchAll,
}

#[derive(Debug, Clone)]
pub struct RouterPath {
    pub method: Method,
    pub url_path: String,
}

impl RouterPath {
    pub fn url_kinds(&self) -> UrlKind {
        let mut i = 0;
        let inner = self.url_path.as_bytes();
        while let Some(&c) = inner.get(i) {
            let is_match_path = c == b'{';
            let is_escaped = c == b'{' && inner.get(i + 1) == Some(&b'{');
            let is_match_all = c == b'{' && inner.get(i + 1) == Some(&b'*');
            if is_match_all {
                return UrlKind::MatchAll;
            } else if is_match_path && !is_escaped {
                return UrlKind::MatchPath;
            } else if is_escaped {
                return UrlKind::Absolutely;
            }
            i += 1;
        }
        UrlKind::Absolutely
    }
}

impl ToString for RouterPath {
    fn to_string(&self) -> String {
        format!("{}:{}", self.method, self.url_path)
    }
}

#[derive(PartialDebug, Default)]
pub struct RouteConfig {
    method: Method,
    url_path: String,
    allowed_platform_roles: HashMap<Platform, HashSet<Role>>,
    ratelimiter: Option<RateLimiter>,
    pub allowed_retry_times: Option<u32>,
}

impl RouteConfig {
    pub fn new_without_config<C: Into<HashMap<Platform, HashSet<Role>>>>(
        method: &Method,
        url_path: &str,
        allowed_roles: C,
    ) -> Self {
        let allowed_roles = allowed_roles.into();
        let url_path = url_path.to_string();
        Self {
            method: method.clone(),
            url_path,
            allowed_platform_roles: allowed_roles,
            ratelimiter: None,
            allowed_retry_times: None,
        }
    }

    pub fn route_path(&self) -> RouterPath {
        RouterPath {
            method: self.method.clone(),
            url_path: self.url_path.clone(),
        }
    }

    pub fn set_route_path(&mut self, new_path: &RouterPath) {
        self.url_path = new_path.url_path.to_string();
    }

    pub fn support_platform(&self, platform: &Platform) -> bool {
        self.allowed_platform_roles.contains_key(platform)
    }

    pub fn distinct_platform(&self) -> HashSet<Platform> {
        self.allowed_platform_roles.keys().cloned().collect()
    }

    pub fn distinct_roles(&self) -> HashSet<Role> {
        self.allowed_platform_roles
            .iter()
            .map(|(_, roles)| roles.iter())
            .flatten()
            .collect::<HashSet<_>>()
            .into_iter()
            .map(Clone::clone)
            .collect()
    }

    pub fn allowed_roles_mut(&mut self) -> &mut HashMap<Platform, HashSet<Role>> {
        &mut self.allowed_platform_roles
    }

    pub fn set_ratelimit(&mut self, max_requests_per_sec: u32) -> &mut Self {
        let rate = Rate::new(Duration::from_secs(1));
        self.ratelimiter = Some(RateLimiter {
            rate,
            max_requests_per_sec: max_requests_per_sec as isize,
        });
        self
    }

    pub fn ratelimit_acquire<K: Hash>(&self, request_key: K, events_number: isize) -> bool {
        if let Some(RateLimiter {
            rate,
            max_requests_per_sec,
        }) = self.ratelimiter.as_ref()
        {
            let current_window_requests = rate.observe(&request_key, events_number);
            if current_window_requests <= *max_requests_per_sec {
                return true;
            }
            return false;
        }
        true
    }

    pub fn ratelimit_max_req_per_seconds(&self) -> Option<u32> {
        self.ratelimiter
            .as_ref()
            .map(|v| v.max_requests_per_sec as u32)
    }

    pub fn is_allowed_roles(&self, platform: &Platform, roles: &HashSet<Role>) -> bool {
        self.allowed_platform_roles
            .get(platform)
            .map(|allowed_roles| allowed_roles.intersection(roles).next().is_some())
            .unwrap_or(false)
    }

    pub fn merge_ratelimiter(&mut self, other: &Self) {
        match (self.ratelimiter.as_ref(), &other.ratelimiter) {
            (None, Some(limiter)) => {
                self.set_ratelimit(limiter.max_requests_per_sec as u32);
            }
            (Some(_), Some(new)) => {
                self.set_ratelimit(new.max_requests_per_sec as u32);
            }
            _ => (),
        };
    }

    pub fn merge_retry_policy(&mut self, other: &Self) {
        match (
            self.allowed_retry_times.as_ref(),
            &other.allowed_retry_times,
        ) {
            (None, Some(retry)) => {
                self.allowed_retry_times = Some(*retry);
            }
            (Some(_), Some(retry)) => {
                self.allowed_retry_times = Some(*retry);
            }
            _ => (),
        };
    }

    pub fn merge_roles(&mut self, other: &Self, overwrite: bool) {
        let other_roles = other.allowed_platform_roles.clone();
        for (platform, roles) in other_roles {
            let platform_roles = self
                .allowed_platform_roles
                .entry(platform)
                .or_insert(HashSet::new());
            if !overwrite {
                platform_roles.extend(roles);
            } else {
                *platform_roles = roles;
            }
        }
    }

    pub fn remove_platform(&mut self, platform: &Platform) {
        self.allowed_platform_roles.remove(&platform);
    }

    pub fn remove_ratelimiter(&mut self) {
        self.ratelimiter.take();
    }

    pub fn is_empty(&self) -> bool {
        self.allowed_platform_roles.is_empty()
    }
}

impl Clone for RouteConfig {
    fn clone(&self) -> Self {
        let ratelimiter = self.ratelimiter.as_ref().map(|rl| {
            let rate = Rate::new(Duration::from_secs(1));
            RateLimiter {
                rate,
                max_requests_per_sec: rl.max_requests_per_sec,
            }
        });
        Self {
            method: self.method.clone(),
            url_path: self.url_path.clone(),
            allowed_platform_roles: self.allowed_platform_roles.clone(),
            ratelimiter,
            allowed_retry_times: self.allowed_retry_times.clone(),
        }
    }
}
