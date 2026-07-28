#![allow(unused_assignments)]

use std::collections::HashMap;

use enum_as_inner::EnumAsInner;
use partialdebug::placeholder::PartialDebug;

use crate::{
    common::constants::http_proxy::LOCALHOST_SERVICE_NAME,
    domain::model::{entity::service_backends::ServiceBackends, value::service::ServiceRegistry},
};

#[derive(Default, PartialDebug, Clone, EnumAsInner)]
pub enum RedirectTarget {
    Service {
        service_name: String,
        service_backends: HashMap<ServiceRegistry, ServiceBackends>,
    },
    #[default]
    Localhost,
}

impl RedirectTarget {
    pub fn get_service_name(&self) -> String {
        match self {
            RedirectTarget::Service { service_name, .. } => service_name.as_str(),
            RedirectTarget::Localhost => LOCALHOST_SERVICE_NAME,
        }
        .to_string()
    }
}
