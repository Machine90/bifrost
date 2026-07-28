use std::{collections::BTreeSet, sync::Arc};

use partialdebug::placeholder::PartialDebug;
use pingora::lb::{
    Backend, Backends,
    selection::{
        BackendSelection as _, UniqueIterator,
        algorithms::RoundRobin,
        weighted::{Weighted, WeightedIterator},
    },
};

const DUMMY_KEY: &[u8] = b"";

#[derive(PartialDebug, Clone)]
pub struct ServiceBackends {
    service_name: String,
    pub(crate) loadbalancer: Arc<Weighted<RoundRobin>>,
    pub(crate) backend_count: usize,
}

impl ServiceBackends {
    pub fn new(service_name: &str, backends: &BTreeSet<Backend>) -> Self {
        let backend_count = backends.len();
        let loadbalancer = Weighted::<RoundRobin>::build(backends);
        Self {
            service_name: service_name.to_string(),
            loadbalancer: Arc::new(loadbalancer),
            backend_count,
        }
    }

    pub fn service_name(&self) -> &str {
        &self.service_name
    }

    pub fn list_backends(&self) -> UniqueIterator<WeightedIterator<RoundRobin>> {
        let selection = Weighted::<RoundRobin>::iter(&self.loadbalancer, DUMMY_KEY);
        UniqueIterator::new(selection, self.backend_count)
    }

    pub fn select(&self, backends: &Backends) -> Option<Backend> {
        let mut iter = self.list_backends();
        while let Some(b) = iter.get_next() {
            if backends.ready(&b) {
                return Some(b);
            }
        }
        None
    }
}
