use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceIsolationMode {
    Host,
    Docker,
}

impl Default for WorkspaceIsolationMode {
    fn default() -> Self {
        Self::Host
    }
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

        Ok(())
    }
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
