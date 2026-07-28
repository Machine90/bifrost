use std::{
    collections::{BTreeSet, HashMap, HashSet},
    hash::Hash,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::{Context, Result};
use http::Method;
use pingora::lb::Backend;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::model::{
    entity::{backend_api_rule::BackendApiRule, privilege_rule::PrivilegeRule},
    value::{platform::Platform, role::Role},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum Endpoint {
    Address(String),
    WeighedAddress { weight: f32, address: String },
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiRule {
    #[serde(alias = "method")]
    methods: String,
    #[serde(alias = "url")]
    path: String,
    #[serde(default, alias = "role")]
    roles: HashSet<String>,
}

impl ApiRule {
    fn methods(&self) -> HashSet<String> {
        let result = self
            .methods
            .split(",")
            .into_iter()
            .map(|m| m.trim().to_string())
            .collect::<HashSet<_>>();
        result
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ServiceRules {
    #[serde(alias = "service")]
    service_name: String,
    #[serde(default)]
    endpoints: Vec<Endpoint>,
    #[serde(default, alias = "rule")]
    api: Vec<ApiRule>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Services {
    services: Vec<ServiceRules>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum InputFile {
    Json(PathBuf),
    Yaml(PathBuf),
    Toml(PathBuf),
}

impl InputFile {
    fn get_path(&self) -> PathBuf {
        match self {
            InputFile::Json(path_buf) => path_buf,
            InputFile::Yaml(path_buf) => path_buf,
            InputFile::Toml(path_buf) => path_buf,
        }
        .clone()
    }

    fn get_file_name(&self) -> String {
        self.get_path()
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or("unknown".to_string())
    }
}

#[derive(Debug, Clone, Default)]
pub struct ServiceConf {
    pub service_privileges: HashMap<String, PrivilegeRule>,
    pub service_backends: HashMap<String, BTreeSet<Backend>>,
}

pub fn read_conf_dir<P: AsRef<Path>>(config_dir: P) -> Result<ServiceConf> {
    let config_dir = config_dir.as_ref().to_path_buf();
    let mut route_files = HashSet::new();
    read_files(config_dir, &mut route_files)?;

    let mut service_privileges = HashMap::new();
    let mut service_backends = HashMap::new();
    let mut services = HashMap::new();
    for file in route_files {
        let config_content = std::fs::read_to_string(file.get_path())?;
        let configs: Value = match file {
            InputFile::Json(_) => serde_json::from_str(&config_content)?,
            InputFile::Yaml(_) => serde_yaml::from_str(&config_content)?,
            InputFile::Toml(_) => toml::from_str(&config_content)?,
        };
        let config_key = file.get_file_name();
        if let Ok(services_configs) = serde_json::from_value::<Services>(configs.clone()) {
            // deserialize as multiple services
            let services_configs = services_configs
                .services
                .into_iter()
                .enumerate()
                .map(|(idx, configs)| {
                    let key = format!("{config_key}#{idx}");
                    (key, configs)
                })
                .collect::<HashMap<_, _>>();
            services.extend(services_configs);
        } else {
            // deserialize as single service
            let service_configs: ServiceRules = serde_json::from_value(configs)
                .context(format!("filename = {config_key}"))
                .context("Failed to decode service configs")?;
            services.insert(config_key, service_configs);
        }
    }
    for (config_key, configs) in services {
        let mut privilege = PrivilegeRule {
            config_key: config_key.clone(),
            backend_apis: vec![],
            platform: Platform::Gateway,
        };
        let service_name = configs.service_name;
        let service_endpoints = configs
            .endpoints
            .into_iter()
            .map(|endpoint| {
                let backend = match endpoint {
                    Endpoint::Address(addr) => Backend::new(&addr),
                    Endpoint::WeighedAddress { weight, address } => {
                        Backend::new_with_weight(&address, (weight * 1000.) as usize)
                    }
                };
                backend
            })
            .filter_map(|backend| backend.ok())
            .collect::<HashSet<_>>();
        service_backends
            .entry(service_name.clone())
            .or_insert(BTreeSet::new())
            .extend(service_endpoints);
        for (idx, api_rules @ ApiRule { path, roles, .. }) in configs.api.iter().enumerate() {
            let mut roles = roles.iter().map(Role::from_str).collect::<HashSet<_>>();
            if roles.is_empty() {
                // at least specific one role anonymous to rule if no roles specified.
                roles.insert(Role::Anonymous);
            }
            for method in api_rules.methods() {
                let rule_key = format!("{config_key}#{idx}#{method}");
                let api = BackendApiRule {
                    key: Some(rule_key),
                    service: service_name.clone(),
                    method: Method::from_str(&method.to_uppercase())
                        .context(format!("method = {method}, path = {path}"))
                        .context("Invalid configure method")?,
                    url_path: path.to_string(),
                    roles: roles.clone(),
                };
                privilege.backend_apis.push(api);
            }
        }
        service_privileges.insert(config_key, privilege);
    }
    Ok(ServiceConf {
        service_backends,
        service_privileges,
    })
}

fn read_files(current_dir: PathBuf, route_files: &mut HashSet<InputFile>) -> Result<()> {
    if !current_dir.exists() {
        return Ok(());
    }
    if current_dir.is_file() {
        if let Some(file) = select_target_file(current_dir) {
            route_files.insert(file);
        }
        return Ok(());
    }
    for dir in current_dir.read_dir()? {
        let dir = dir?;
        let next_path = dir.path();
        read_files(next_path, route_files)?;
    }
    Ok(())
}

fn select_target_file(file: PathBuf) -> Option<InputFile> {
    if let Some(ext) = file.extension() {
        let ext = ext.to_string_lossy().to_string();
        let file = match ext.as_str() {
            "json" => InputFile::Json(file),
            "yaml" | "yml" => InputFile::Yaml(file),
            "toml" => InputFile::Toml(file),
            _ => return None,
        };
        return Some(file);
    }
    None
}
