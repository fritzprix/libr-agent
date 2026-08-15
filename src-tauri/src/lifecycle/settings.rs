use serde::Deserialize;

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemSettings {
    pub search_index_frequency_minutes: Option<u64>,
    pub web_action_timeout_seconds: Option<u64>,
    pub http_server_port: Option<u16>,
    pub http_server_expose: Option<bool>,
    pub scheduled_task_minimum_interval_minutes: Option<u64>,
    /// When true (default), inhibit idle sleep while LibrAgent is running.
    pub prevent_sleep_during_agent_work: Option<bool>,
}

impl SystemSettings {
    pub fn prevent_sleep_during_agent_work_or_default(&self) -> bool {
        self.prevent_sleep_during_agent_work.unwrap_or(true)
    }
}
