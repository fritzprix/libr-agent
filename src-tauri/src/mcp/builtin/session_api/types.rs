use serde::Deserialize;
use serde_json::Value;
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

impl MessageSummaryOptions {
    pub fn from_args(args: &Value) -> Self {
        let summary_only = args
            .get("summaryOnly")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let include_raw_preview = args
            .get("includeRawPreview")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let preview_limit = args
            .get("previewLimit")
            .and_then(|v| v.as_u64())
            .map(|v| v.clamp(1, 10) as usize)
            .unwrap_or(3);
        let skip_if_unchanged = args
            .get("skipIfUnchanged")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let min_interval_seconds = args
            .get("minIntervalSeconds")
            .and_then(|v| v.as_u64())
            .map(|v| v.min(120))
            .unwrap_or(5);
        let forced_rest_seconds = args
            .get("forcedRestSeconds")
            .and_then(|v| v.as_u64())
            .map(|v| v.min(300))
            .unwrap_or(20);
        let rapid_call_threshold = args
            .get("rapidCallThreshold")
            .and_then(|v| v.as_u64())
            .map(|v| v.clamp(2, 10) as u32)
            .unwrap_or(3);

        MessageSummaryOptions {
            summary_only,
            include_raw_preview,
            preview_limit,
            skip_if_unchanged,
            min_interval_seconds,
            forced_rest_seconds,
            rapid_call_threshold,
        }
    }
}
