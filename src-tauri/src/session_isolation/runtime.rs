use tokio::process::Child;

use super::path_mapper::PathMappingLayer;
use super::types::ShellType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellDialect {
    Bash,
    Sh,
    PowerShell,
}

#[derive(Debug)]
pub struct SpawnedShell {
    pub child: Child,
    pub initial_cwd: String,
    pub path_mapper: PathMappingLayer,
    pub shell_type: ShellType,
    pub shell_dialect: ShellDialect,
}
