use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

pub const DEFAULT_DOCKER_WORKDIR: &str = "/workspace";

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
    /// Image for managed LibrAgent Docker sessions. Optional when attaching to an
    /// existing container (`attach_container`).
    #[serde(default)]
    pub image: Option<String>,
    /// Existing Docker container id/name to attach instead of creating one.
    #[serde(default)]
    pub attach_container: Option<String>,
    /// Container working directory / file-tool root. Defaults to `/workspace`.
    #[serde(default)]
    pub workdir: Option<String>,
    /// When false, session cleanup must not stop/remove the container.
    /// Defaults to true for managed containers.
    #[serde(default)]
    pub manage_lifecycle: Option<bool>,
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
    pub fn is_attach(&self) -> bool {
        self.attach_container
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
    }

    pub fn attach_container_name(&self) -> Option<&str> {
        self.attach_container
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub fn image_ref(&self) -> Option<&str> {
        self.image
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub fn workdir(&self) -> &str {
        self.workdir
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(DEFAULT_DOCKER_WORKDIR)
    }

    pub fn manage_lifecycle(&self) -> bool {
        self.manage_lifecycle.unwrap_or(true)
    }

    pub fn validate(&self) -> Result<(), String> {
        let has_attach = self.is_attach();
        let has_image = self.image_ref().is_some();

        if !has_attach && !has_image {
            return Err(
                "Docker config requires either 'image' (managed) or 'attachContainer' (attach)"
                    .to_string(),
            );
        }

        if has_attach {
            let workdir = self.workdir();
            if !workdir.starts_with('/') {
                return Err(format!(
                    "Docker workdir must be an absolute Unix path, got '{workdir}'"
                ));
            }
            if workdir.contains('\0') {
                return Err("Docker workdir must not contain NUL bytes".to_string());
            }
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
