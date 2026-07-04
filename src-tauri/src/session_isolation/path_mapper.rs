use path_clean::PathClean;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathMappingLayer {
    host_workspace: PathBuf,
    container_workspace: PathBuf,
}

impl PathMappingLayer {
    pub fn new(host_workspace: PathBuf) -> Self {
        Self {
            host_workspace,
            container_workspace: PathBuf::from("/workspace"),
        }
    }

    pub fn host_workspace(&self) -> &Path {
        &self.host_workspace
    }

    pub fn container_workspace(&self) -> &Path {
        &self.container_workspace
    }

    pub fn container_to_host(&self, container_path: &str) -> Option<PathBuf> {
        let normalized = PathBuf::from(container_path.replace('\\', "/")).clean();

        if normalized == self.container_workspace {
            return Some(self.host_workspace.clone());
        }

        if normalized.starts_with(&self.container_workspace) {
            let relative = normalized.strip_prefix(&self.container_workspace).ok()?;
            return Some(self.host_workspace.join(relative));
        }

        None
    }
}
