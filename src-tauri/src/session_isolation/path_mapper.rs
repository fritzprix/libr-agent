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

        // Strict prefix matching against "/workspace"
        if normalized == "/workspace" {
            return Some(self.host_workspace.clone());
        }

        if let Some(relative_str) = normalized.strip_prefix("/workspace/") {
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
}
