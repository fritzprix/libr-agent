use async_trait::async_trait;
use reqwest::{Client, Method};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::RwLock as TokioRwLock;
use tokio::time::sleep;

use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{MCPContent, MCPResult, ServiceContext};
use crate::mcp::MCPTool;
use crate::repositories::settings_repository::SettingsRepository;
use crate::repositories::MessageRepository;
use crate::repositories::SessionRepository;
use crate::state::get_settings_repository;

pub mod tools;

static MESSAGE_FETCH_CACHE: OnceLock<TokioRwLock<HashMap<String, MessageFetchCacheEntry>>> =
    OnceLock::new();

#[derive(Debug, Clone)]
struct MessageFetchCacheEntry {
    digest: u64,
    last_checked_at: Instant,
    rapid_call_count: u32,
    cooldown_until: Option<Instant>,
}

fn message_fetch_cache() -> &'static TokioRwLock<HashMap<String, MessageFetchCacheEntry>> {
    MESSAGE_FETCH_CACHE.get_or_init(|| TokioRwLock::new(HashMap::new()))
}

#[derive(Debug, Default)]
pub struct SessionApiServer;

#[derive(Debug, Clone, Copy)]
struct MessageSummaryOptions {
    summary_only: bool,
    include_raw_preview: bool,
    preview_limit: usize,
    skip_if_unchanged: bool,
    min_interval_seconds: u64,
    forced_rest_seconds: u64,
    rapid_call_threshold: u32,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SystemSettings {
    http_server_port: Option<u16>,
}

impl SessionApiServer {
    const SWARM_CONTEXT_NODE_LIMIT: usize = 40;
    const SWARM_CONTEXT_PREVIEW_LIMIT: usize = 20;
    const SWARM_MESSAGE_PREVIEW_MAX_CHARS: usize = 140;

    pub fn new() -> Self {
        Self
    }

    pub fn metadata_static() -> crate::mcp::types::BuiltinServerMetadata {
        crate::mcp::types::BuiltinServerMetadata {
            display_name: "Session API".to_string(),
            description: "Client tools for the internal Session Management HTTP API".to_string(),
            icon: None,
        }
    }

    pub fn tools_static() -> Vec<MCPTool> {
        tools::all_tools()
    }

    fn extract_assistant_description(config: &Value) -> String {
        if let Some(description) = config.get("description").and_then(|v| v.as_str()) {
            let cleaned = description.trim();
            if !cleaned.is_empty() {
                return Self::truncate_text(cleaned, 140);
            }
        }

        if let Some(system_prompt) = config.get("systemPrompt").and_then(|v| v.as_str()) {
            let first_meaningful_line = system_prompt
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or("");

            if !first_meaningful_line.is_empty() {
                return Self::truncate_text(first_meaningful_line, 140);
            }
        }

        "No description".to_string()
    }

    async fn base_url(&self) -> String {
        let settings_repo = get_settings_repository();

        let port = match settings_repo.get("systemSettings").await {
            Ok(Some(model)) => serde_json::from_str::<SystemSettings>(&model.value)
                .ok()
                .and_then(|s| s.http_server_port)
                .unwrap_or(3030),
            _ => 3030,
        };

        format!("http://127.0.0.1:{}", port)
    }

    fn http_client(&self) -> Result<Client, String> {
        Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {e}"))
    }

    async fn call_json(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        query: Option<Vec<(String, String)>>,
    ) -> Result<Value, String> {
        let client = self.http_client()?;
        let url = format!("{}{}", self.base_url().await, path);

        let mut req = client.request(method, &url);

        if let Some(q) = query {
            req = req.query(&q);
        }

        if let Some(b) = body {
            req = req.json(&b);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("Session API request failed: {e}"))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read response body: {e}"))?;

        if !status.is_success() {
            return Err(format!(
                "Session API {} {} failed ({status}): {}",
                path,
                status.as_u16(),
                text
            ));
        }

        serde_json::from_str(&text).map_err(|e| format!("Invalid JSON response: {e}. body={text}"))
    }

    fn success_result(text: String, data: Value) -> MCPResult {
        MCPResult {
            content: Some(vec![MCPContent::Text {
                text,
                is_error: None,
            }]),
            structured_content: Some(data),
            is_error: Some(false),
        }
    }

    fn read_required_string(args: &Value, key: &str) -> Result<String, String> {
        args.get(key)
            .and_then(|v| v.as_str())
            .map(|v| v.to_string())
            .ok_or_else(|| format!("Missing required parameter: {key}"))
    }

    fn resolve_parent_session_id(
        provided_parent: Option<&str>,
        caller_session_id: Option<&str>,
    ) -> Result<Option<String>, String> {
        let normalized = provided_parent
            .map(str::trim)
            .filter(|value| !value.is_empty());

        match caller_session_id {
            Some(caller_id) => Ok(Some(caller_id.to_string())),
            None => match normalized {
                None => Ok(None),
                Some(value) if value.eq_ignore_ascii_case("current") => Err(
                    "parentSessionId='current' requires caller session context. Provide an explicit parentSessionId or call from within a session.".to_string(),
                ),
                Some(value) => Ok(Some(value.to_string())),
            },
        }
    }

    async fn collect_descendant_snapshot(
        root_session_id: &str,
        max_nodes: usize,
    ) -> Result<(Vec<(String, String, String, usize, Option<String>)>, bool), String> {
        let repo = crate::state::get_session_repository();
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        let mut visited: HashSet<String> = HashSet::new();
        let mut rows: Vec<(String, String, String, usize, Option<String>)> = Vec::new();
        let mut truncated = false;

        queue.push_back((root_session_id.to_string(), 0));
        visited.insert(root_session_id.to_string());

        'bfs: while let Some((parent_id, parent_depth)) = queue.pop_front() {
            let child_ids = repo
                .get_child_session_ids(&parent_id)
                .await
                .map_err(|e| format!("Failed to fetch child sessions for {}: {}", parent_id, e))?;

            for child_id in child_ids {
                if !visited.insert(child_id.clone()) {
                    continue;
                }

                let (name, status) = match repo.get_session(&child_id).await {
                    Ok(Some(child)) => (
                        child.name.unwrap_or_else(|| "Unnamed".to_string()),
                        child.status.as_str().to_string(),
                    ),
                    Ok(None) => ("Unknown".to_string(), "unknown".to_string()),
                    Err(_) => ("Unknown".to_string(), "unknown".to_string()),
                };

                let preview = if rows.len() < Self::SWARM_CONTEXT_PREVIEW_LIMIT {
                    Self::latest_assistant_preview_for_session(
                        &child_id,
                        Self::SWARM_MESSAGE_PREVIEW_MAX_CHARS,
                    )
                    .await
                } else {
                    None
                };

                rows.push((child_id.clone(), name, status, parent_depth + 1, preview));

                if rows.len() >= max_nodes {
                    truncated = true;
                    break 'bfs;
                }

                queue.push_back((child_id, parent_depth + 1));
            }
        }

        Ok((rows, truncated))
    }

    fn build_swarm_snapshot_text(
        root_session_id: &str,
        rows: &[(String, String, String, usize, Option<String>)],
        truncated: bool,
        max_nodes: usize,
    ) -> String {
        if rows.is_empty() {
            return format!(
                "Swarm board: no active sub-agents under current command session {}.\nNext step: use createChildSession to spawn a worker.",
                root_session_id
            );
        }

        let direct_count = rows
            .iter()
            .filter(|(_, _, _, depth, _)| *depth == 1)
            .count();
        let total_count = rows.len();

        let mut status_counts: HashMap<String, usize> = HashMap::new();
        for (_, _, status, _, _) in rows {
            *status_counts.entry(status.clone()).or_insert(0) += 1;
        }

        let mut status_parts = status_counts
            .iter()
            .map(|(status, count)| format!("{}:{}", status, count))
            .collect::<Vec<_>>();
        status_parts.sort();

        let mut text = format!(
            "Swarm command board (commander session: {})\n- Direct units: {}\n- Total descendants: {}\n- Status breakdown: {}\n\nUnit roster:\n",
            root_session_id,
            direct_count,
            total_count,
            status_parts.join(", ")
        );

        for (session_id, name, status, depth, preview) in rows {
            let indent = "  ".repeat(depth.saturating_sub(1));
            let mut line = format!(
                "- {}{} (ID: {}) status={} depth={}\n",
                indent, name, session_id, status, depth
            );

            if let Some(summary) = preview {
                line.push_str(&format!("  {}latest assistant: {}\n", indent, summary));
            }

            text.push_str(&line);
        }

        if truncated {
            text.push_str(&format!(
                "\nRoster truncated at {} units. Use specific session IDs with getMessages/getSession for deeper checks.",
                max_nodes
            ));
        }

        text
    }

    async fn latest_assistant_preview_for_session(
        session_id: &str,
        max_chars: usize,
    ) -> Option<String> {
        let repo = crate::state::get_message_repository();
        let messages = repo.get_messages_by_session(session_id, 10).await.ok()?;

        for message in messages {
            if message.role != "assistant" {
                continue;
            }

            for item in message.content {
                if let MCPContent::Text { text, .. } = item {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        return Some(Self::truncate_text(trimmed, max_chars));
                    }
                }
            }

            return Some("[assistant message has no text content]".to_string());
        }

        None
    }

    fn truncate_text(input: &str, max_chars: usize) -> String {
        let normalized = input.replace('\n', " ").trim().to_string();
        if normalized.chars().count() <= max_chars {
            return normalized;
        }

        let mut truncated = String::new();
        for ch in normalized.chars().take(max_chars) {
            truncated.push(ch);
        }
        truncated.push_str("...");
        truncated
    }

    fn message_preview_text(message: &Value, options: MessageSummaryOptions) -> Option<String> {
        let role = message
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let message_id = message
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let content = message.get("content").and_then(|v| v.as_array())?;
        let text_snippet_limit = if options.include_raw_preview {
            260
        } else {
            120
        };
        let line_snippet_limit = if options.include_raw_preview {
            300
        } else {
            160
        };

        let mut snippets: Vec<String> = Vec::new();
        for item in content {
            let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match item_type {
                "text" => {
                    if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                        let text = text.trim();
                        if !text.is_empty() {
                            snippets.push(Self::truncate_text(text, text_snippet_limit));
                        }
                    }
                }
                "tool_call" => {
                    let tool_name = item
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    snippets.push(format!("[tool_call:{}]", tool_name));
                }
                _ => {}
            }
        }

        let preview = if snippets.is_empty() {
            "[non-text content]".to_string()
        } else {
            Self::truncate_text(&snippets.join(" | "), line_snippet_limit)
        };

        if options.summary_only {
            Some(format!("• [{}] {}", role, preview))
        } else {
            Some(format!("• [{}] {} :: {}", role, message_id, preview))
        }
    }

    fn build_messages_summary(
        messages: &[Value],
        session_id: &str,
        options: MessageSummaryOptions,
    ) -> String {
        let message_count = messages.len();
        if message_count == 0 {
            return format!("Fetched 0 messages for session {}", session_id);
        }

        let previews = messages
            .iter()
            .take(options.preview_limit)
            .filter_map(|message| Self::message_preview_text(message, options))
            .collect::<Vec<_>>();

        if previews.is_empty() {
            return format!(
                "Fetched {} messages for session {}",
                message_count, session_id
            );
        }

        let mode_hint = if options.summary_only {
            "summary-only"
        } else {
            "expanded"
        };

        format!(
            "Fetched {} messages for session {} (mode: {})\n\nRecent message previews (latest first):\n{}",
            message_count,
            session_id,
            mode_hint,
            previews.join("\n")
        )
    }

    fn extract_session_status(session: &Value) -> String {
        session
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string()
    }

    fn is_terminal_status(status: &str) -> bool {
        matches!(
            status.to_ascii_lowercase().as_str(),
            "idle" | "terminated" | "failed" | "error"
        )
    }

    fn latest_assistant_message_text(
        messages: &[Value],
        max_chars: Option<usize>,
    ) -> Option<(String, String)> {
        for message in messages {
            let role = message.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if role != "assistant" {
                continue;
            }

            let message_id = message
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let content = match message.get("content").and_then(|v| v.as_array()) {
                Some(content) => content,
                None => continue,
            };

            for item in content {
                let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if item_type != "text" {
                    continue;
                }

                if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                    let text = text.trim();
                    if !text.is_empty() {
                        let output = match max_chars {
                            Some(limit) if limit > 0 => Self::truncate_text(text, limit),
                            _ => text.to_string(),
                        };
                        return Some((message_id, output));
                    }
                }
            }

            return Some((
                message_id,
                "[assistant message has no text content]".to_string(),
            ));
        }

        None
    }

    async fn wait_until_session_terminal(
        &self,
        session_id: &str,
        timeout_seconds: u64,
        poll_interval_seconds: u64,
    ) -> Result<(Value, u64), String> {
        let timeout_seconds = timeout_seconds.clamp(5, 900);
        let poll_interval_seconds = poll_interval_seconds.clamp(1, 30);

        let started_at = Instant::now();
        let mut poll_count: u64 = 0;

        loop {
            let session = self
                .call_json(
                    Method::GET,
                    &format!("/api/sessions/{}", session_id),
                    None,
                    None,
                )
                .await?;

            poll_count = poll_count.saturating_add(1);
            let status = Self::extract_session_status(&session);
            if Self::is_terminal_status(&status) {
                return Ok((session, poll_count));
            }

            if started_at.elapsed() >= Duration::from_secs(timeout_seconds) {
                return Err(format!(
                    "waitForSessionIdle timed out after {}s for session {}",
                    timeout_seconds, session_id
                ));
            }

            sleep(Duration::from_secs(poll_interval_seconds)).await;
        }
    }

    fn read_message_summary_options(args: &Value) -> MessageSummaryOptions {
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

    fn compute_messages_digest(messages: &[Value]) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();

        for message in messages {
            message
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .hash(&mut hasher);

            message
                .get("updatedAt")
                .or_else(|| message.get("updated_at"))
                .or_else(|| message.get("createdAt"))
                .or_else(|| message.get("created_at"))
                .map(|v| v.to_string())
                .unwrap_or_else(|| "0".to_string())
                .hash(&mut hasher);

            message
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .hash(&mut hasher);
        }

        hasher.finish()
    }

    fn message_cache_key(
        caller_session_id: Option<&str>,
        target_session_id: &str,
        limit: Option<u64>,
    ) -> String {
        let caller = caller_session_id.unwrap_or("no-caller");
        let limit_text = limit
            .map(|value| value.to_string())
            .unwrap_or_else(|| "default".to_string());
        format!("{}::{}::{}", caller, target_session_id, limit_text)
    }

    async fn unchanged_messages_notice(
        messages: &[Value],
        caller_session_id: Option<&str>,
        target_session_id: &str,
        limit: Option<u64>,
    ) -> Option<String> {
        let digest = Self::compute_messages_digest(messages);
        let key = Self::message_cache_key(caller_session_id, target_session_id, limit);

        let mut cache = message_fetch_cache().write().await;
        if cache.len() > 2048 {
            cache.clear();
        }

        let now = std::time::Instant::now();
        let entry = cache.entry(key).or_insert(MessageFetchCacheEntry {
            digest,
            last_checked_at: now,
            rapid_call_count: 0,
            cooldown_until: None,
        });

        let previous = entry.digest;
        entry.digest = digest;
        entry.last_checked_at = now;

        if previous == digest {
            Some(format!(
                "Fetched {} messages for session {}\n\nNo new message changes since last fetch. Skip repeated ingestion and continue with current context.",
                messages.len(),
                target_session_id
            ))
        } else {
            None
        }
    }

    async fn min_interval_notice(
        caller_session_id: Option<&str>,
        target_session_id: &str,
        limit: Option<u64>,
        options: MessageSummaryOptions,
    ) -> Option<String> {
        if options.min_interval_seconds == 0 && options.forced_rest_seconds == 0 {
            return None;
        }

        let key = Self::message_cache_key(caller_session_id, target_session_id, limit);
        let mut cache = message_fetch_cache().write().await;

        let now = std::time::Instant::now();
        let entry = cache.entry(key).or_insert(MessageFetchCacheEntry {
            digest: 0,
            last_checked_at: now,
            rapid_call_count: 0,
            cooldown_until: None,
        });

        if let Some(cooldown_until) = entry.cooldown_until {
            if now < cooldown_until {
                let wait_seconds = cooldown_until.duration_since(now).as_secs().max(1);
                entry.last_checked_at = now;
                return Some(format!(
                    "Forced cooldown active for session {}.\n\nPlease wait {}s before calling getMessages again.",
                    target_session_id, wait_seconds
                ));
            }
            entry.cooldown_until = None;
            entry.rapid_call_count = 0;
        }

        let elapsed = now.duration_since(entry.last_checked_at).as_secs();
        if options.min_interval_seconds > 0 && elapsed < options.min_interval_seconds {
            entry.rapid_call_count = entry.rapid_call_count.saturating_add(1);
            entry.last_checked_at = now;

            if options.forced_rest_seconds > 0
                && entry.rapid_call_count >= options.rapid_call_threshold
            {
                let cooldown_until = now + Duration::from_secs(options.forced_rest_seconds);
                entry.cooldown_until = Some(cooldown_until);
                entry.rapid_call_count = 0;
                return Some(format!(
                    "Too many rapid getMessages calls detected for session {}.\n\nForced cooldown started: {}s. Let the model rest before polling again.",
                    target_session_id, options.forced_rest_seconds
                ));
            }

            let wait_seconds = options.min_interval_seconds - elapsed;
            return Some(format!(
                "Skipped getMessages for session {} to preserve context budget.\n\nPlease wait {}s before polling again (minIntervalSeconds={}; rapidCount={}/{}).",
                target_session_id,
                wait_seconds,
                options.min_interval_seconds,
                entry.rapid_call_count,
                options.rapid_call_threshold
            ));
        }

        entry.last_checked_at = now;
        entry.rapid_call_count = 0;
        None
    }
}

#[async_trait]
impl BuiltinMCPServer for SessionApiServer {
    fn name(&self) -> &str {
        "session_api"
    }

    fn description(&self) -> &str {
        "Client tools for internal HTTP Session Management API"
    }

    fn tools(&self) -> Vec<MCPTool> {
        tools::all_tools()
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        caller_session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        match tool_name {
            "healthCheck" => {
                let data = self
                    .call_json(Method::GET, "/api/health", None, None)
                    .await?;
                Ok(Self::success_result(
                    "Session API health check succeeded.".to_string(),
                    data,
                ))
            }
            "createChildSession" => {
                let assistant_id = Self::read_required_string(&args, "assistantId")?;
                let request = Self::read_required_string(&args, "request")?;

                let parent_session_id = Self::resolve_parent_session_id(
                    args.get("parentSessionId").and_then(|v| v.as_str()),
                    caller_session_id.as_deref(),
                )?
                .ok_or_else(|| {
                    "Missing parent session context: provide explicit parentSessionId or call from within a parent session"
                        .to_string()
                })?;

                let mut body = json!({
                    "parentSessionId": parent_session_id,
                    "assistantId": assistant_id,
                    "request": request,
                });

                if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
                    body["name"] = Value::String(name.to_string());
                }

                if let Some(path) = args.get("workspacePath").and_then(|v| v.as_str()) {
                    body["workspacePath"] = Value::String(path.to_string());
                }

                if let Some(max_depth) = args.get("maxDepth").and_then(|v| v.as_u64()) {
                    body["maxDepth"] = Value::Number(max_depth.into());
                }

                if let Some(max_fanout) = args.get("maxFanout").and_then(|v| v.as_u64()) {
                    body["maxFanout"] = Value::Number(max_fanout.into());
                }

                let data = self
                    .call_json(Method::POST, "/api/sessions", Some(body), None)
                    .await?;

                let child_id = data.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
                let depth = data.get("depth").and_then(|v| v.as_u64()).unwrap_or(0);
                let lineage = data
                    .get("lineageId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                Ok(Self::success_result(
                    format!(
                        "Child session created: {} (parent: {}, depth: {}, lineage: {})",
                        child_id, parent_session_id, depth, lineage
                    ),
                    data,
                ))
            }
            "getSession" => {
                let session_id = Self::read_required_string(&args, "sessionId")?;
                let data = self
                    .call_json(
                        Method::GET,
                        &format!("/api/sessions/{}", session_id),
                        None,
                        None,
                    )
                    .await?;
                Ok(Self::success_result(
                    format!("Fetched session: {}", session_id),
                    data,
                ))
            }
            "waitForSessionIdle" => {
                let session_id = Self::read_required_string(&args, "sessionId")?;

                let timeout_seconds = args
                    .get("timeoutSeconds")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(180);
                let poll_interval_seconds = args
                    .get("pollIntervalSeconds")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(3);
                let include_last_assistant_message = args
                    .get("includeLastAssistantMessage")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let result_message_limit = args
                    .get("resultMessageLimit")
                    .and_then(|v| v.as_u64())
                    .map(|v| v.clamp(1, 200))
                    .unwrap_or(20);
                let assistant_message_max_chars = args
                    .get("assistantMessageMaxChars")
                    .and_then(|v| v.as_u64())
                    .map(|v| v.min(200000) as usize)
                    .filter(|v| *v > 0);

                let (session_data, poll_count) = self
                    .wait_until_session_terminal(
                        &session_id,
                        timeout_seconds,
                        poll_interval_seconds,
                    )
                    .await?;

                let final_status = Self::extract_session_status(&session_data);

                if !include_last_assistant_message {
                    return Ok(Self::success_result(
                        format!(
                            "Session {} reached terminal status '{}' after {} polls.",
                            session_id, final_status, poll_count
                        ),
                        json!({
                            "session": session_data,
                            "status": final_status,
                            "pollCount": poll_count,
                            "messages": Value::Null
                        }),
                    ));
                }

                let messages_data = self
                    .call_json(
                        Method::GET,
                        &format!("/api/sessions/{}/messages", session_id),
                        None,
                        Some(vec![(
                            "limit".to_string(),
                            result_message_limit.to_string(),
                        )]),
                    )
                    .await?;

                let messages = messages_data
                    .get("messages")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();

                let text = if let Some((message_id, assistant_text)) =
                    Self::latest_assistant_message_text(&messages, assistant_message_max_chars)
                {
                    format!(
                        "Session {} reached terminal status '{}' after {} polls.\n\nLatest assistant result [{}]:\n{}",
                        session_id, final_status, poll_count, message_id, assistant_text
                    )
                } else {
                    format!(
                        "Session {} reached terminal status '{}' after {} polls.\n\nNo assistant text message was found in the latest {} messages.",
                        session_id, final_status, poll_count, result_message_limit
                    )
                };

                Ok(Self::success_result(
                    text,
                    json!({
                        "session": session_data,
                        "status": final_status,
                        "pollCount": poll_count,
                        "messages": messages_data
                    }),
                ))
            }
            "getMessages" => {
                let target_session_id = Self::read_required_string(&args, "sessionId")?;

                let requested_limit = args.get("limit").and_then(|v| v.as_u64());
                let options = Self::read_message_summary_options(&args);

                if options.skip_if_unchanged {
                    if let Some(wait_notice) = Self::min_interval_notice(
                        caller_session_id.as_deref(),
                        &target_session_id,
                        requested_limit,
                        options,
                    )
                    .await
                    {
                        return Ok(Self::success_result(
                            wait_notice,
                            json!({
                                "sessionId": target_session_id,
                                "skipped": true,
                                "reason": "min_interval",
                                "minIntervalSeconds": options.min_interval_seconds,
                                "forcedRestSeconds": options.forced_rest_seconds,
                                "rapidCallThreshold": options.rapid_call_threshold
                            }),
                        ));
                    }
                }

                let query = requested_limit.map(|v| vec![("limit".to_string(), v.to_string())]);

                let data = self
                    .call_json(
                        Method::GET,
                        &format!("/api/sessions/{}/messages", target_session_id),
                        None,
                        query,
                    )
                    .await?;

                let messages = data
                    .get("messages")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();

                if options.skip_if_unchanged {
                    if let Some(unchanged_notice) = Self::unchanged_messages_notice(
                        &messages,
                        caller_session_id.as_deref(),
                        &target_session_id,
                        requested_limit,
                    )
                    .await
                    {
                        return Ok(Self::success_result(unchanged_notice, data));
                    }
                }

                let summary_text =
                    Self::build_messages_summary(&messages, &target_session_id, options);

                Ok(Self::success_result(summary_text, data))
            }
            "getChildSessions" => {
                let parent_session_id = Self::read_required_string(&args, "parentSessionId")?;

                let data = self
                    .call_json(
                        Method::GET,
                        &format!("/api/sessions/{}/children", parent_session_id),
                        None,
                        None,
                    )
                    .await?;

                let child_ids = data
                    .get("children")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|value| value.as_str().map(str::to_string))
                    .collect::<Vec<_>>();

                let repo = crate::state::get_session_repository();
                let mut rows: Vec<(String, String, String, Option<String>)> = Vec::new();

                for child_id in &child_ids {
                    let (name, status) = match repo.get_session(child_id).await {
                        Ok(Some(child)) => (
                            child.name.unwrap_or_else(|| "Unnamed".to_string()),
                            child.status.as_str().to_string(),
                        ),
                        Ok(None) => ("Unknown".to_string(), "unknown".to_string()),
                        Err(_) => ("Unknown".to_string(), "unknown".to_string()),
                    };

                    let preview = Self::latest_assistant_preview_for_session(
                        child_id,
                        Self::SWARM_MESSAGE_PREVIEW_MAX_CHARS,
                    )
                    .await;

                    rows.push((child_id.clone(), name, status, preview));
                }

                let mut message = format!(
                    "Fetched {} direct sub-agents for commander session {}",
                    child_ids.len(),
                    parent_session_id
                );

                if rows.is_empty() {
                    message.push_str(
                        "\n\nNo direct sub-agents online. Next step: createChildSession to deploy a worker.",
                    );
                } else {
                    message.push_str("\n\nDirect unit roster:\n");
                    for (child_id, name, status, preview) in rows {
                        message.push_str(&format!(
                            "- {} (ID: {}) status={}\n",
                            name, child_id, status
                        ));
                        if let Some(summary) = preview {
                            message.push_str(&format!("  latest assistant: {}\n", summary));
                        }
                    }
                }

                Ok(Self::success_result(message, data))
            }
            "sendMessage" => {
                let session_id = Self::read_required_string(&args, "sessionId")?;
                let content = Self::read_required_string(&args, "content")?;

                let data = self
                    .call_json(
                        Method::POST,
                        &format!("/api/sessions/{}/messages", session_id),
                        Some(json!({ "content": content })),
                        None,
                    )
                    .await?;

                let message_id = data.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
                let status = data
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                Ok(Self::success_result(
                    format!("Message accepted: {} (status: {})", message_id, status),
                    data,
                ))
            }
            "terminateSession" => {
                let session_id = Self::read_required_string(&args, "sessionId")?;
                let data = self
                    .call_json(
                        Method::POST,
                        &format!("/api/sessions/{}/terminate", session_id),
                        None,
                        None,
                    )
                    .await?;

                Ok(Self::success_result(
                    format!("Terminated session: {}", session_id),
                    data,
                ))
            }
            "listAssistants" => {
                let data = self
                    .call_json(Method::GET, "/api/assistants", None, None)
                    .await?;

                // Extract assistant details for text output (AI agents need to see this!)
                let assistants_text = data
                    .get("assistants")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|a| {
                                let name = a.get("name")?.as_str()?;
                                let id = a.get("id")?.as_str()?;

                                // Parse config (might be string or object)
                                let config = a.get("config")?;
                                let parsed_config = if let Some(config_str) = config.as_str() {
                                    serde_json::from_str::<Value>(config_str).ok()?
                                } else {
                                    config.clone()
                                };

                                // Extract description from config
                                let description =
                                    Self::extract_assistant_description(&parsed_config);

                                Some(format!(
                                    "• {} [ID: {}]\n  Description: {}",
                                    name, id, description
                                ))
                            })
                            .collect::<Vec<_>>()
                            .join("\n\n")
                    })
                    .unwrap_or_default();

                let assistant_count = data
                    .get("assistants")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.len())
                    .unwrap_or(0);

                let text = if assistant_count == 0 {
                    "No assistants found".to_string()
                } else {
                    format!(
                        "Found {} {}:\n\n{}",
                        assistant_count,
                        if assistant_count == 1 {
                            "assistant"
                        } else {
                            "assistants"
                        },
                        assistants_text
                    )
                };

                Ok(Self::success_result(text, data))
            }
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }

    async fn get_service_context(&self, options: Option<&Value>) -> ServiceContext {
        let base_url = self.base_url().await;

        // 1. Base prompt
        let mut context_prompt = format!(
            "## Session API\n\nInternal API client is available at {}\nUse these tools to create/manage nested sessions.",
            base_url
        );

        // 2. Fetch swarm snapshot if session_id is provided
        if let Some(opts) = options {
            if let Some(session_id) = opts.get("sessionId").and_then(|v| v.as_str()) {
                match Self::collect_descendant_snapshot(session_id, Self::SWARM_CONTEXT_NODE_LIMIT)
                    .await
                {
                    Ok((rows, truncated)) => {
                        context_prompt.push_str("\n\n### Swarm Snapshot\n");
                        context_prompt.push_str(&Self::build_swarm_snapshot_text(
                            session_id,
                            &rows,
                            truncated,
                            Self::SWARM_CONTEXT_NODE_LIMIT,
                        ));
                        context_prompt.push_str("\n\nUse `session_api` tools to communicate with specific sub-agents or poll their messages.");
                    }
                    Err(e) => {
                        log::warn!("Failed to fetch child sessions for context: {}", e);
                    }
                }
            }
        }

        ServiceContext {
            context_prompt,
            structured_state: Some(json!({
                "base_url": base_url,
                "server": "session_api"
            })),
        }
    }
}
