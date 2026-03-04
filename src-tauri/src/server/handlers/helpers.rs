use std::collections::HashMap;
use std::sync::OnceLock;
use tokio::sync::RwLock as TokioRwLock;

use super::types::SessionLineageMeta;

pub static SESSION_LINEAGE: OnceLock<TokioRwLock<HashMap<String, SessionLineageMeta>>> =
    OnceLock::new();

pub fn lineage_store() -> &'static TokioRwLock<HashMap<String, SessionLineageMeta>> {
    SESSION_LINEAGE.get_or_init(|| TokioRwLock::new(HashMap::new()))
}

/// Returns true if the path points to a restricted system directory that agents
/// should not be allowed to use as a workspace.
pub fn is_restricted_system_path(path: &std::path::Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        let restricted_prefixes = [
            "c:\\windows",
            "c:\\program files",
            "c:\\program files (x86)",
            "c:\\programdata",
            "c:\\system volume information",
        ];

        let path_components: Vec<_> = path.components().collect();

        for prefix in restricted_prefixes.iter() {
            let prefix_components: Vec<_> = std::path::Path::new(prefix).components().collect();

            if path_components.len() < prefix_components.len() {
                continue;
            }

            let mut matches = true;
            for (p_comp, pref_comp) in path_components.iter().zip(prefix_components.iter()) {
                use std::path::{Component, Prefix};

                let p_disk = match p_comp {
                    Component::Prefix(p) => match p.kind() {
                        Prefix::Disk(d) | Prefix::VerbatimDisk(d) => Some(d.to_ascii_lowercase()),
                        _ => None,
                    },
                    _ => None,
                };

                let pref_disk = match pref_comp {
                    Component::Prefix(p) => match p.kind() {
                        Prefix::Disk(d) | Prefix::VerbatimDisk(d) => Some(d.to_ascii_lowercase()),
                        _ => None,
                    },
                    _ => None,
                };

                if let (Some(d1), Some(d2)) = (p_disk, pref_disk) {
                    if d1 != d2 {
                        matches = false;
                        break;
                    }
                } else {
                    let p_str = p_comp.as_os_str().to_string_lossy().to_lowercase();
                    let pref_str = pref_comp.as_os_str().to_string_lossy().to_lowercase();
                    if p_str != pref_str {
                        matches = false;
                        break;
                    }
                }
            }

            if matches {
                return true;
            }
        }

        false
    }

    #[cfg(not(target_os = "windows"))]
    {
        // macOS and Linux file systems are often case-insensitive by default or case-preserving.
        // Lowercase the path to ensure safe, case-insensitive component matching across Unix OSes.
        let path_lower = std::path::PathBuf::from(path.to_string_lossy().to_lowercase());

        let restricted_prefixes = [
            "/etc",
            "/sys",
            "/proc",
            "/dev",
            "/run",
            "/boot",
            "/bin",
            "/sbin",
            "/lib",
            "/lib64",
            "/usr/bin",
            "/usr/sbin",
            "/usr/lib",
            "/system",  // macOS
            "/library", // macOS
        ];

        for prefix in restricted_prefixes.iter() {
            if path_lower.starts_with(prefix) {
                return true;
            }
        }

        false
    }
}
