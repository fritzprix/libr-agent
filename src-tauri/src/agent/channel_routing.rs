#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelRouteCandidate {
    pub session_id: String,
    pub session_name: String,
    pub parent_session_id: Option<String>,
}

pub fn resolve_auto_routed_channel_target(
    server_name: &str,
    mut candidates: Vec<ChannelRouteCandidate>,
) -> Result<ChannelRouteCandidate, String> {
    match candidates.len() {
        0 => Err(format!(
            "No active session is currently connected to channel server '{}'",
            server_name
        )),
        1 => Ok(candidates.remove(0)),
        _ => {
            candidates.sort_by(|left, right| left.session_name.cmp(&right.session_name));
            let candidate_list = candidates
                .iter()
                .map(|candidate| format!("{} ({})", candidate.session_name, candidate.session_id))
                .collect::<Vec<_>>()
                .join(", ");

            Err(format!(
                "Ambiguous active sessions for channel server '{}': {}. Use the session-scoped channel endpoint to target a specific session.",
                server_name, candidate_list
            ))
        }
    }
}
