use enum_as_inner::EnumAsInner;
use enum_kinds::EnumKind;

const STATIC_REGISTRY_TYPE_NAME: &str = "static";
#[cfg(feature = "nacos")]
const NACOS_REGISTRY_TYPE_NAME: &str = "nacos";

#[derive(Debug, Clone, PartialEq, Eq, Hash, EnumAsInner, EnumKind)]
#[enum_kind(ServiceRegistry, derive(EnumAsInner, Hash))]
pub enum Service {
    Static(String),
    #[cfg(feature = "nacos")]
    Nacos(String),
}

impl Service {
    pub fn get_name(&self) -> &str {
        match self {
            Service::Static(svc) => svc.as_str(),
            #[cfg(feature = "nacos")]
            Service::Nacos(svc) => svc.as_str(),
        }
    }

    pub fn get_registry(&self) -> ServiceRegistry {
        match self {
            Service::Static(_) => ServiceRegistry::Static,
            #[cfg(feature = "nacos")]
            Service::Nacos(_) => ServiceRegistry::Nacos,
        }
    }
}

impl ServiceRegistry {
    pub fn into_service(&self, name: &str) -> Service {
        match self {
            ServiceRegistry::Static => Service::Static(name.to_string()),
            #[cfg(feature = "nacos")]
            ServiceRegistry::Nacos => Service::Nacos(name.to_string()),
        }
    }

    pub fn get_name(&self) -> &str {
        match self {
            Self::Static => STATIC_REGISTRY_TYPE_NAME,
            #[cfg(feature = "nacos")]
            Self::Nacos => NACOS_REGISTRY_TYPE_NAME,
        }
    }
}
