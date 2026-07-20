use path_clean::PathClean;
use std::path::{Path, PathBuf};

use crate::models::workspace_isolation::DEFAULT_DOCKER_WORKDIR;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathMappingLayer {
    host_workspace: PathBuf,
    container_workspace: PathBuf,
}

impl PathMappingLayer {
    pub fn new(host_workspace: PathBuf) -> Self {
        Self::with_container_root(host_workspace, DEFAULT_DOCKER_WORKDIR)
    }

    pub fn with_container_root(
        host_workspace: PathBuf,
        container_workspace: impl AsRef<Path>,
    ) -> Self {
        Self {
            host_workspace,
            container_workspace: PathBuf::from(container_workspace.as_ref()),
        }
    }

    pub fn host_workspace(&self) -> &Path {
        &self.host_workspace
    }

    pub fn container_workspace(&self) -> &Path {
        &self.container_workspace
    }

    fn container_root_unix(&self) -> String {
        self.container_workspace
            .to_string_lossy()
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_string()
    }

    pub fn container_to_host(&self, container_path: &str) -> Option<PathBuf> {
        // Normalize container path to Unix style with forward slashes
        let path_str = container_path.replace('\\', "/");

        // Clean the path to resolve relative dot segments (like '/workspace/../outside')
        // We clean using PathBuf, then convert back to Unix string for safe comparison
        let normalized = if path_str.starts_with('/') {
            PathBuf::from(&path_str)
                .clean()
                .to_string_lossy()
                .replace('\\', "/")
        } else {
            let absolute = format!("/{}", path_str);
            PathBuf::from(&absolute)
                .clean()
                .to_string_lossy()
                .replace('\\', "/")
        };

        let root = self.container_root_unix();
        if normalized == root {
            return Some(self.host_workspace.clone());
        }

        let prefix = format!("{root}/");
        if let Some(relative_str) = normalized.strip_prefix(&prefix) {
            if relative_str.is_empty() {
                return Some(self.host_workspace.clone());
            }
            return Some(self.host_workspace.join(relative_str));
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_to_host_mapping() {
        let host = PathBuf::from(if cfg!(windows) {
            r"C:\Users\test\project"
        } else {
            "/home/test/project"
        });
        let mapper = PathMappingLayer::new(host.clone());

        // Exact match
        assert_eq!(mapper.container_to_host("/workspace"), Some(host.clone()));
        assert_eq!(mapper.container_to_host("/workspace/"), Some(host.clone()));

        // Under workspace
        assert_eq!(
            mapper.container_to_host("/workspace/src/main.rs"),
            Some(host.join("src").join("main.rs"))
        );
        assert_eq!(
            mapper.container_to_host(r"/workspace\src\main.rs"),
            Some(host.join("src").join("main.rs"))
        );

        // Outside workspace or relative breakout attempts
        assert_eq!(mapper.container_to_host("/workspace/../outside"), None);
        assert_eq!(mapper.container_to_host("/other/path"), None);
    }

    #[test]
    fn test_container_to_host_mapping_custom_root() {
        let host = PathBuf::from("/tmp/staging");
        let mapper = PathMappingLayer::with_container_root(host.clone(), "/app");

        assert_eq!(mapper.container_to_host("/app"), Some(host.clone()));
        assert_eq!(
            mapper.container_to_host("/app/gpt2.c"),
            Some(host.join("gpt2.c"))
        );
        assert_eq!(mapper.container_to_host("/workspace/gpt2.c"), None);
    }
}
