use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceIsolationMode {
    #[default]
    Host,
    Docker,
}

impl WorkspaceIsolationMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Docker => "docker",
        }
    }
}

impl fmt::Display for WorkspaceIsolationMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for WorkspaceIsolationMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "host" => Ok(Self::Host),
            "docker" => Ok(Self::Docker),
            other => Err(format!("Invalid workspace isolation mode: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DockerWorkspaceConfig {
    pub image: String,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub port_bindings: Vec<DockerPortBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct DockerPortBinding {
    pub container_port: u16,
    pub host_port: Option<u16>,
}

impl DockerWorkspaceConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.image.trim().is_empty() {
            return Err("Docker image cannot be empty".to_string());
        }

        for (key, value) in &self.env {
            validate_env_key(key)?;
            validate_env_value(key, value)?;
        }

        validate_port_bindings(&self.port_bindings)?;

        Ok(())
    }
}

pub fn validate_port_bindings(bindings: &[DockerPortBinding]) -> Result<(), String> {
    let mut container_ports = std::collections::HashSet::new();
    let mut host_ports = std::collections::HashSet::new();

    for binding in bindings {
        validate_port("containerPort", binding.container_port)?;
        if !container_ports.insert(binding.container_port) {
            return Err(format!(
                "Duplicate Docker container port binding: {}",
                binding.container_port
            ));
        }

        if let Some(host_port) = binding.host_port {
            validate_port("hostPort", host_port)?;
            if !host_ports.insert(host_port) {
                return Err(format!("Duplicate Docker host port binding: {host_port}"));
            }
        }
    }

    Ok(())
}

fn validate_port(name: &str, port: u16) -> Result<(), String> {
    if port == 0 {
        return Err(format!("{name} must be between 1 and 65535"));
    }
    Ok(())
}

pub fn validate_env_value(key: &str, value: &str) -> Result<(), String> {
    if value.contains('\0') {
        return Err(format!(
            "Invalid Docker env value for '{key}': NUL bytes are not allowed"
        ));
    }

    Ok(())
}

pub fn validate_env_key(key: &str) -> Result<(), String> {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return Err("Docker env key cannot be empty".to_string());
    };

    if !(first == '_' || first.is_ascii_alphabetic()) {
        return Err(format!(
            "Invalid Docker env key '{key}': must start with a letter or underscore"
        ));
    }

    if chars.any(|ch| !(ch == '_' || ch.is_ascii_alphanumeric())) {
        return Err(format!(
            "Invalid Docker env key '{key}': only letters, numbers, and underscores are allowed"
        ));
    }

    Ok(())
}
