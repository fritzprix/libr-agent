//! Matcher semantics plus the event labels and working directory a hook
//! run uses.

use crate::services::hooks::envelope::SessionStartSource;
use crate::session::{get_session_manager, resolve_session_workspace_dir};
use regex::Regex;
use std::path::PathBuf;

pub fn matcher_matches(matcher: Option<&str>, value: &str) -> bool {
    let Some(pattern) = matcher.map(str::trim).filter(|s| !s.is_empty()) else {
        return true;
    };
    match Regex::new(pattern) {
        Ok(re) => re.is_match(value),
        Err(e) => {
            log::warn!("invalid hook matcher regex {:?}: {e}", pattern);
            false
        }
    }
}

pub(super) fn session_start_source_label(source: SessionStartSource) -> &'static str {
    match source {
        SessionStartSource::Startup => "startup",
        SessionStartSource::Resume => "resume",
        SessionStartSource::Clear => "clear",
        SessionStartSource::Compact => "compact",
    }
}

pub(super) async fn resolve_session_cwd(
    session_id: &str,
    override_cwd: Option<PathBuf>,
) -> Result<PathBuf, String> {
    if let Some(p) = override_cwd {
        return Ok(p);
    }
    let session_manager = get_session_manager()?;
    resolve_session_workspace_dir(session_manager, session_id).await
}
