use std::path::{Path, PathBuf};
use tokio::time::Instant;

#[derive(Clone, Debug, serde::Serialize)]
pub struct SessionWorkspaceInfo {
    pub session_id: String,
    #[serde(serialize_with = "serialize_pathbuf")]
    pub workspace_path: PathBuf,
    #[serde(serialize_with = "serialize_option_pathbuf")]
    pub workspace_override: Option<PathBuf>,
    #[serde(serialize_with = "serialize_instant")]
    pub created_at: Instant,
    #[serde(serialize_with = "serialize_instant")]
    pub last_accessed: Instant,
    pub is_template: bool,
}

fn serialize_pathbuf<S>(path: &Path, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&path.to_string_lossy())
}

fn serialize_option_pathbuf<S>(path: &Option<PathBuf>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match path {
        Some(p) => serializer.serialize_str(&p.to_string_lossy()),
        None => serializer.serialize_none(),
    }
}

fn serialize_instant<S>(instant: &Instant, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let duration_since_start = instant.elapsed();
    serializer.serialize_u64(duration_since_start.as_secs())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionStats {
    pub total_sessions: usize,
    pub active_sessions: usize,
    pub pool_sessions: usize,
}
