use serde::Deserialize;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct MessageFetchCacheEntry {
    pub digest: u64,
    pub last_checked_at: Instant,
    pub rapid_call_count: u32,
    pub cooldown_until: Option<Instant>,
}

#[derive(Debug, Clone, Copy)]
pub struct MessageSummaryOptions {
    pub summary_only: bool,
    pub include_raw_preview: bool,
    pub preview_limit: usize,
    pub skip_if_unchanged: bool,
    pub min_interval_seconds: u64,
    pub forced_rest_seconds: u64,
    pub rapid_call_threshold: u32,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SystemSettings {
    pub http_server_port: Option<u16>,
}
