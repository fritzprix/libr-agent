use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Timeout a hook handler gets when its config sets none, in milliseconds (#3005).
///
/// Claude Code defaults a `command` hook to 600 s, lowers that to 30 s on
/// `UserPromptSubmit`, and gives `SessionEnd` hooks a 1.5 s budget.
/// <https://code.claude.com/docs/en/hooks#timeouts>
pub fn default_hook_timeout_ms(event: HookEvent) -> u64 {
    match event {
        HookEvent::UserPromptSubmit => 30_000,
        HookEvent::SessionEnd => 1_500,
        _ => 600_000,
    }
}

/// Safety net for a handler that carries no resolved timeout.
pub const DEFAULT_HOOK_TIMEOUT_MS: u64 = 600_000;

const RESERVED_UNKNOWN_EVENTS: &[(&str, &str)] = &[
    (
        "PostCompact",
        "PostCompact is not supported; use SessionStart with source=compact",
    ),
    (
        "postCompact",
        "PostCompact is not supported; use SessionStart with source=compact",
    ),
    (
        "SubagentStart",
        "SubagentStart is reserved for a later schema version",
    ),
    (
        "subagentStart",
        "SubagentStart is reserved for a later schema version",
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HookEvent {
    SessionStart,
    SessionEnd,
    UserPromptSubmit,
    PreToolUse,
    PostToolUse,
    Stop,
    SubagentStop,
    PreCompact,
    Notification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookDispatchMode {
    Execute,
    Stub,
}

impl HookEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::Stop => "Stop",
            Self::SubagentStop => "SubagentStop",
            Self::PreCompact => "PreCompact",
            Self::Notification => "Notification",
        }
    }

    pub fn dispatch_mode(self) -> HookDispatchMode {
        HookDispatchMode::Execute
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookHandler {
    pub command: String,
    /// Exec-form argument vector (`args` in the hook config). `Some`, even when
    /// empty, selects exec form: the command is spawned directly, without a
    /// shell (#3005).
    pub args: Option<Vec<String>>,
    pub powershell: Option<String>,
    pub timeout_ms: u64,
    pub r#async: bool,
    pub shell: Option<String>,
    /// Set for `type: "http"` handlers, which POST the event envelope to
    /// `url` instead of spawning a command (#3009).
    pub http: Option<HookHttpConfig>,
    pub r#if: Option<String>,
}

/// HTTP hook handler fields from the Hooks reference: the POST target, extra
/// headers whose `$VAR` values are interpolated only for names listed in
/// `allowed_env_vars` (#3009).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HookHttpConfig {
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub allowed_env_vars: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookMatcherGroup {
    pub matcher: Option<String>,
    pub handlers: Vec<HookHandler>,
}

/// Summary of an unrecognized/newer hook event preserved for install preview
/// so plugins declaring newer Claude Code events install cleanly instead of
/// failing validation (#2925).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedHookEntry {
    pub matcher: Option<String>,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HooksConfig {
    pub version: Option<u32>,
    pub events: BTreeMap<HookEvent, Vec<HookMatcherGroup>>,
    pub unsupported_events: BTreeMap<String, Vec<UnsupportedHookEntry>>,
    /// Handlers of not-yet-implemented types (`prompt`, `mcp_tool`, `agent`,
    /// or unknown types) on supported events, keyed by event name. They
    /// install without failing and are marked unsupported in the preview
    /// (#3009).
    pub unsupported_handlers: BTreeMap<String, Vec<UnsupportedHookEntry>>,
}

pub fn load_hooks_from_root(root: &Path) -> Result<HooksConfig, String> {
    let hooks_dir = root.join("hooks");
    let claude_path = hooks_dir.join("hooks.json");
    let cursor_path = hooks_dir.join("hooks-cursor.json");

    let mut merged = HooksConfig {
        version: None,
        events: BTreeMap::new(),
        unsupported_events: BTreeMap::new(),
        unsupported_handlers: BTreeMap::new(),
    };
    let mut found_any = false;

    if claude_path.is_file() {
        let piece = parse_hooks_file(&claude_path)?;
        merge_hooks_config(&mut merged, piece)?;
        found_any = true;
    }
    if cursor_path.is_file() {
        let piece = parse_hooks_file(&cursor_path)?;
        merge_hooks_config(&mut merged, piece)?;
        found_any = true;
    }

    if !found_any {
        return Err(format!(
            "no hook config found under {}",
            hooks_dir.display()
        ));
    }

    Ok(merged)
}

/// Load and merge explicit hook config files — the custom `hooks` paths a
/// `.claude-plugin/plugin.json` manifest declares (#2921). The files
/// replace the default `hooks/hooks.json` discovery, so at least one must
/// exist (callers validate existence before calling).
pub fn load_hooks_from_config_files(files: &[PathBuf]) -> Result<HooksConfig, String> {
    if files.is_empty() {
        return Err("no hook config files given".to_string());
    }
    let mut merged = HooksConfig {
        version: None,
        events: BTreeMap::new(),
        unsupported_events: BTreeMap::new(),
        unsupported_handlers: BTreeMap::new(),
    };
    for file in files {
        let piece = parse_hooks_file(file)?;
        merge_hooks_config(&mut merged, piece)?;
    }
    Ok(merged)
}

fn merge_hooks_config(dst: &mut HooksConfig, src: HooksConfig) -> Result<(), String> {
    if dst.version.is_none() {
        dst.version = src.version;
    }
    for (event, groups) in src.events {
        dst.events.entry(event).or_default().extend(groups);
    }
    for (raw_event, entries) in src.unsupported_events {
        dst.unsupported_events
            .entry(raw_event)
            .or_default()
            .extend(entries);
    }
    for (event, entries) in src.unsupported_handlers {
        dst.unsupported_handlers
            .entry(event)
            .or_default()
            .extend(entries);
    }
    Ok(())
}

impl HooksConfig {
    /// An empty config: a plugin that declares no hook config at all.
    pub fn empty() -> Self {
        Self {
            version: None,
            events: BTreeMap::new(),
            unsupported_events: BTreeMap::new(),
            unsupported_handlers: BTreeMap::new(),
        }
    }
}

/// Parse the inline hooks objects a manifest declares and merge them into
/// `dst` (#3005). Every object has the same shape as a hook config file's
/// `"hooks"` member.
pub fn merge_hooks_inline(
    dst: &mut HooksConfig,
    values: &[Value],
    label: &str,
) -> Result<(), String> {
    for value in values {
        let parsed = parse_hooks_inline(value, label)?;
        merge_hooks_config(dst, parsed)?;
    }
    Ok(())
}

fn parse_hooks_file(path: &Path) -> Result<HooksConfig, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let root: Value =
        serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))?;
    parse_hooks_value(&root, path)
}

/// Parse an inline `hooks` object that a plugin manifest carries instead of a
/// file path (#3005). The value has the same shape as the `"hooks"` member of a
/// hook config file, so it is wrapped and parsed by the file parser.
pub fn parse_hooks_inline(value: &Value, label: &str) -> Result<HooksConfig, String> {
    let wrapped = serde_json::json!({ "hooks": value });
    parse_hooks_value(&wrapped, Path::new(label))
}

fn parse_hooks_value(root: &Value, path: &Path) -> Result<HooksConfig, String> {
    let obj = root
        .as_object()
        .ok_or_else(|| format!("{}: root must be a JSON object", path.display()))?;

    let version = obj
        .get("version")
        .and_then(|v| v.as_u64())
        .map(|v| u32::try_from(v).map_err(|_| "version out of range".to_string()))
        .transpose()?;

    let hooks_value = obj
        .get("hooks")
        .ok_or_else(|| format!("{}: missing \"hooks\" object", path.display()))?;

    let hooks_obj = hooks_value
        .as_object()
        .ok_or_else(|| format!("{}: \"hooks\" must be an object", path.display()))?;

    let mut events = BTreeMap::new();
    let mut unsupported_events = BTreeMap::new();
    let mut unsupported_handlers = BTreeMap::new();

    for (raw_key, groups_value) in hooks_obj {
        match normalize_event_key(raw_key) {
            Ok(event) => {
                let (groups, unsupported) = parse_matcher_groups(groups_value, event, path)?;
                events.insert(event, groups);
                if !unsupported.is_empty() {
                    log::info!(
                        "{}: recording {} unsupported hook handler type(s) on {}",
                        path.display(),
                        unsupported.len(),
                        event.as_str()
                    );
                    unsupported_handlers.insert(event.as_str().to_string(), unsupported);
                }
            }
            Err(_) => {
                let note = reserved_event_message(raw_key).unwrap_or("event not supported yet");
                log::info!(
                    "{}: recording unsupported hook event {:?} ({note})",
                    path.display(),
                    raw_key
                );
                let entries = collect_unsupported_entries(groups_value, note);
                unsupported_events.insert(raw_key.clone(), entries);
            }
        }
    }

    Ok(HooksConfig {
        version,
        events,
        unsupported_events,
        unsupported_handlers,
    })
}

fn collect_unsupported_entries(value: &Value, fallback_note: &str) -> Vec<UnsupportedHookEntry> {
    let mut out = Vec::new();
    if let Some(arr) = value.as_array() {
        for group in arr {
            let matcher = group
                .get("matcher")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            if let Some(hooks) = group.get("hooks").and_then(|v| v.as_array()) {
                for h in hooks {
                    let cmd = h
                        .get("command")
                        .and_then(|v| v.as_str())
                        .unwrap_or(fallback_note)
                        .to_string();
                    out.push(UnsupportedHookEntry {
                        matcher: matcher.clone(),
                        command: cmd,
                    });
                }
            } else if let Some(cmd) = group.get("command").and_then(|v| v.as_str()) {
                out.push(UnsupportedHookEntry {
                    matcher,
                    command: cmd.to_string(),
                });
            }
        }
    }
    if out.is_empty() {
        out.push(UnsupportedHookEntry {
            matcher: None,
            command: format!("({fallback_note})"),
        });
    }
    out
}

fn reserved_event_message(key: &str) -> Option<&'static str> {
    RESERVED_UNKNOWN_EVENTS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, msg)| *msg)
}

fn normalize_event_key(key: &str) -> Result<HookEvent, String> {
    match key {
        "SessionStart" | "sessionStart" => Ok(HookEvent::SessionStart),
        "SessionEnd" | "sessionEnd" => Ok(HookEvent::SessionEnd),
        "UserPromptSubmit" | "userPromptSubmit" | "beforeSubmitPrompt" => {
            Ok(HookEvent::UserPromptSubmit)
        }
        "PreToolUse" | "preToolUse" => Ok(HookEvent::PreToolUse),
        "PostToolUse" | "postToolUse" => Ok(HookEvent::PostToolUse),
        "Stop" | "stop" => Ok(HookEvent::Stop),
        "SubagentStop" | "subagentStop" => Ok(HookEvent::SubagentStop),
        "PreCompact" | "preCompact" => Ok(HookEvent::PreCompact),
        "Notification" | "notification" => Ok(HookEvent::Notification),
        _ => Err("event name is not in the supported v1 set".to_string()),
    }
}

fn parse_matcher_groups(
    value: &Value,
    event: HookEvent,
    path: &Path,
) -> Result<(Vec<HookMatcherGroup>, Vec<UnsupportedHookEntry>), String> {
    let arr = value
        .as_array()
        .ok_or_else(|| format!("{}: {} must be an array", path.display(), event.as_str()))?;

    let mut groups = Vec::with_capacity(arr.len());
    let mut unsupported = Vec::new();
    for (idx, entry) in arr.iter().enumerate() {
        groups.push(parse_matcher_group(
            entry,
            event,
            path,
            idx,
            &mut unsupported,
        )?);
    }
    Ok((groups, unsupported))
}

fn parse_matcher_group(
    value: &Value,
    event: HookEvent,
    path: &Path,
    idx: usize,
    unsupported: &mut Vec<UnsupportedHookEntry>,
) -> Result<HookMatcherGroup, String> {
    let obj = value.as_object().ok_or_else(|| {
        format!(
            "{}: {}[{}] must be an object",
            path.display(),
            event.as_str(),
            idx
        )
    })?;

    let matcher = match obj.get("matcher") {
        None => None,
        Some(v) => parse_optional_string(v, "matcher")?,
    };

    let mut handlers = if let Some(nested) = obj.get("hooks") {
        let arr = nested.as_array().ok_or_else(|| {
            format!(
                "{}: {}[{}].hooks must be an array",
                path.display(),
                event.as_str(),
                idx
            )
        })?;
        let mut out = Vec::with_capacity(arr.len());
        for (h_idx, h) in arr.iter().enumerate() {
            out.push(parse_handler(
                h,
                event,
                &format!("{}[{}].hooks[{}]", event.as_str(), idx, h_idx),
                path,
            )?);
        }
        out
    } else if obj.contains_key("command") || obj.contains_key("type") {
        vec![parse_handler(
            value,
            event,
            &format!("{}[{}]", event.as_str(), idx),
            path,
        )?]
    } else {
        return Err(format!(
            "{}: {}[{}] must contain \"hooks\" or a command handler",
            path.display(),
            event.as_str(),
            idx
        ));
    };

    // Record handler types LibrAgent does not run yet (#3009); install must
    // succeed with the handler marked in the preview instead of failing.
    let mut supported = Vec::with_capacity(handlers.len());
    for parsed in handlers.drain(..) {
        match parsed {
            ParsedHandler::Handler(handler) => supported.push(handler),
            ParsedHandler::Unsupported { handler_type } => unsupported.push(UnsupportedHookEntry {
                matcher: matcher.clone(),
                command: format!("(handler type \"{handler_type}\" is not supported yet)"),
            }),
        }
    }

    Ok(HookMatcherGroup {
        matcher,
        handlers: supported,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawHandler {
    #[serde(rename = "type")]
    handler_type: Option<String>,
    command: Option<String>,
    args: Option<Vec<String>>,
    powershell: Option<String>,
    timeout: Option<u64>,
    timeout_ms: Option<u64>,
    r#async: Option<bool>,
    shell: Option<String>,
    url: Option<String>,
    headers: Option<BTreeMap<String, String>>,
    allowed_env_vars: Option<Vec<String>>,
    #[serde(rename = "if")]
    if_condition: Option<String>,
}

enum ParsedHandler {
    Handler(HookHandler),
    Unsupported { handler_type: String },
}

fn parse_handler(
    value: &Value,
    event: HookEvent,
    label: &str,
    path: &Path,
) -> Result<ParsedHandler, String> {
    let raw: RawHandler = serde_json::from_value(value.clone())
        .map_err(|e| format!("{}: invalid handler at {label}: {e}", path.display()))?;

    let handler_type = raw.handler_type.unwrap_or_else(|| "command".to_string());

    // Deliberate decision per type (#3009): `prompt` and `agent` need a
    // model turn per hook event and `mcp_tool` needs the session's MCP
    // client context, none of which the hook dispatcher holds; they install
    // as unsupported. `http` is implemented below.
    if !matches!(handler_type.as_str(), "command" | "http") {
        return Ok(ParsedHandler::Unsupported { handler_type });
    }

    let http = if handler_type == "http" {
        let url = raw.url.ok_or_else(|| {
            format!(
                "{}: {label}: \"url\" is required for http handlers",
                path.display()
            )
        })?;
        // Reject unparsable or non-http(s) URLs at install (#3041 G1.5), so
        // a typo fails the preview instead of silently failing the hook on
        // every event.
        let parsed = url::Url::parse(&url).map_err(|e| {
            format!(
                "{}: {label}: http handler \"url\" is not a valid URL ({e}): {url}",
                path.display()
            )
        })?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(format!(
                "{}: {label}: http handler \"url\" must use http or https, got {:?}: {url}",
                path.display(),
                parsed.scheme()
            ));
        }
        Some(HookHttpConfig {
            url,
            headers: raw.headers.unwrap_or_default(),
            allowed_env_vars: raw.allowed_env_vars.unwrap_or_default(),
        })
    } else {
        None
    };

    let command = match &http {
        Some(_) => String::new(),
        None => raw
            .command
            .ok_or_else(|| format!("{}: {label}: \"command\" is required", path.display()))?,
    };

    let timeout_ms = resolve_timeout(event, raw.timeout, raw.timeout_ms)
        .map_err(|e| format!("{}: {label}: {e}", path.display()))?;

    let async_flag = raw.r#async.unwrap_or(false);
    if event == HookEvent::SessionStart && async_flag {
        return Err(format!(
            "{}: {label}: SessionStart hooks cannot use async: true",
            path.display()
        ));
    }

    // Exec form takes `command` as an executable, so extra words belong in
    // `args`; the spawn would otherwise look for a binary with a space in its
    // name (Claude Code warns about the same shape).
    if raw.args.is_some()
        && command.split_whitespace().count() > 1
        && !command.contains(std::path::MAIN_SEPARATOR)
        && !command.contains('/')
    {
        log::warn!(
            "{}: {label}: exec-form command {:?} contains whitespace; move the extra tokens into \"args\"",
            path.display(),
            command
        );
    }

    let if_condition = raw
        .if_condition
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Ok(ParsedHandler::Handler(HookHandler {
        command,
        args: raw.args,
        powershell: raw.powershell,
        timeout_ms,
        r#async: async_flag,
        shell: raw.shell,
        http,
        r#if: if_condition,
    }))
}

/// Resolve a handler timeout to milliseconds. An explicit `timeout` (seconds)
/// wins over `timeoutMs`; otherwise the event default applies (#3005).
fn resolve_timeout(
    event: HookEvent,
    timeout: Option<u64>,
    timeout_ms: Option<u64>,
) -> Result<u64, String> {
    if let Some(secs) = timeout {
        if secs == 0 {
            return Err("timeout must be greater than 0".to_string());
        }
        return Ok(secs.saturating_mul(1000));
    }
    if let Some(ms) = timeout_ms {
        if ms == 0 {
            return Err("timeoutMs must be greater than 0".to_string());
        }
        return Ok(ms);
    }
    Ok(default_hook_timeout_ms(event))
}

fn parse_optional_string(value: &Value, field: &str) -> Result<Option<String>, String> {
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_str()
        .map(|s| Some(s.to_string()))
        .ok_or_else(|| format!("{field} must be a string"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn superpowers_claude_json() -> &'static str {
        r#"{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "startup|clear|compact",
        "hooks": [
          {
            "type": "command",
            "command": "${CLAUDE_PLUGIN_ROOT}/hooks/run-hook.cmd session-start",
            "powershell": "& \"${CLAUDE_PLUGIN_ROOT}/hooks/run-hook.cmd\" session-start",
            "shell": "bash",
            "async": false
          }
        ]
      }
    ]
  }
}"#
    }

    #[test]
    fn parses_superpowers_session_start() {
        let cfg = parse_hooks_value(
            &serde_json::from_str(superpowers_claude_json()).unwrap(),
            Path::new("test.json"),
        )
        .unwrap();
        let groups = cfg.events.get(&HookEvent::SessionStart).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].matcher.as_deref(), Some("startup|clear|compact"));
        let h = &groups[0].handlers[0];
        assert!(h.powershell.is_some());
        assert_eq!(
            h.timeout_ms,
            default_hook_timeout_ms(HookEvent::SessionStart)
        );
        assert_eq!(
            HookEvent::SessionStart.dispatch_mode(),
            HookDispatchMode::Execute
        );
        assert_eq!(
            HookEvent::PreToolUse.dispatch_mode(),
            HookDispatchMode::Execute
        );
    }

    #[test]
    fn cursor_camel_case_normalizes_to_session_start() {
        let json = r#"{
  "version": 1,
  "hooks": {
    "sessionStart": [
      { "command": "./hooks/run-hook.cmd session-start" }
    ]
  }
}"#;
        let cfg = parse_hooks_value(
            &serde_json::from_str(json).unwrap(),
            Path::new("hooks-cursor.json"),
        )
        .unwrap();
        assert!(cfg.events.contains_key(&HookEvent::SessionStart));
    }

    #[test]
    fn timeout_ms_alias_is_kept_verbatim() {
        let json = r#"{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          { "type": "command", "command": "echo ok", "timeoutMs": 1500 }
        ]
      }
    ]
  }
}"#;
        let cfg =
            parse_hooks_value(&serde_json::from_str(json).unwrap(), Path::new("t.json")).unwrap();
        assert_eq!(
            cfg.events[&HookEvent::SessionStart][0].handlers[0].timeout_ms,
            1500
        );
    }

    #[test]
    fn default_timeouts_follow_the_claude_code_values() {
        let json = r#"{
  "hooks": {
    "PreToolUse": [{ "command": "echo pre" }],
    "UserPromptSubmit": [{ "command": "echo prompt" }],
    "SessionEnd": [{ "command": "echo end" }]
  }
}"#;
        let cfg =
            parse_hooks_value(&serde_json::from_str(json).unwrap(), Path::new("t.json")).unwrap();
        let timeout = |event| cfg.events[&event][0].handlers[0].timeout_ms;
        assert_eq!(timeout(HookEvent::PreToolUse), 600_000);
        assert_eq!(timeout(HookEvent::UserPromptSubmit), 30_000);
        assert_eq!(timeout(HookEvent::SessionEnd), 1_500);
    }

    #[test]
    fn explicit_timeout_seconds_win_over_the_event_default() {
        let json = r#"{
  "hooks": {
    "SessionEnd": [{ "command": "echo end", "timeout": 20 }]
  }
}"#;
        let cfg =
            parse_hooks_value(&serde_json::from_str(json).unwrap(), Path::new("t.json")).unwrap();
        assert_eq!(
            cfg.events[&HookEvent::SessionEnd][0].handlers[0].timeout_ms,
            20_000
        );
    }

    #[test]
    fn args_select_exec_form_and_are_kept_in_order() {
        let json = r#"{
  "hooks": {
    "PreToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "${CLAUDE_PLUGIN_ROOT}/scripts/format.js",
            "args": ["--fix", "a file.txt"]
          }
        ]
      }
    ]
  }
}"#;
        let cfg =
            parse_hooks_value(&serde_json::from_str(json).unwrap(), Path::new("t.json")).unwrap();
        assert_eq!(
            cfg.events[&HookEvent::PreToolUse][0].handlers[0]
                .args
                .as_deref(),
            Some(["--fix".to_string(), "a file.txt".to_string()].as_slice())
        );
    }

    #[test]
    fn without_args_the_handler_stays_in_shell_form() {
        let json = r#"{
  "hooks": {
    "PreToolUse": [{ "command": "echo hi | tr a-z A-Z" }]
  }
}"#;
        let cfg =
            parse_hooks_value(&serde_json::from_str(json).unwrap(), Path::new("t.json")).unwrap();
        assert!(cfg.events[&HookEvent::PreToolUse][0].handlers[0]
            .args
            .is_none());
    }

    #[test]
    fn records_unknown_and_newer_events_as_unsupported_without_failing() {
        for key in [
            "PostCompact",
            "SubagentStart",
            "PostToolUseFailure",
            "PermissionRequest",
            "bogusEvent",
        ] {
            let json = format!(
                r#"{{"hooks":{{"{key}":[{{"matcher":"Bash","hooks":[{{"type":"command","command":"echo hi"}}]}}],"SessionStart":[{{"hooks":[{{"type":"command","command":"echo start"}}]}}]}}}}"#
            );
            let cfg = parse_hooks_value(&serde_json::from_str(&json).unwrap(), Path::new("t.json"))
                .expect("unknown/newer hook events should not fail install");
            assert!(cfg.events.contains_key(&HookEvent::SessionStart));
            let entries = cfg
                .unsupported_events
                .get(key)
                .expect("unsupported event should be recorded");
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].matcher.as_deref(), Some("Bash"));
            assert_eq!(entries[0].command, "echo hi");
        }
    }

    #[test]
    fn records_unsupported_handler_types_without_failing_install() {
        // #3009: prompt/mcp_tool/agent and unknown types on supported events
        // install and are recorded as unsupported instead of failing.
        for handler_type in ["prompt", "mcp_tool", "agent", "bogus"] {
            let json = format!(
                r#"{{"hooks":{{"PreToolUse":[{{"matcher":"Bash","hooks":[{{"type":"{handler_type}"}}]}}],"SessionStart":[{{"hooks":[{{"type":"command","command":"echo start"}}]}}]}}}}"#
            );
            let cfg = parse_hooks_value(&serde_json::from_str(&json).unwrap(), Path::new("t.json"))
                .expect("unsupported handler types must not fail install");
            assert!(cfg.events.contains_key(&HookEvent::SessionStart));
            assert!(
                cfg.events[&HookEvent::PreToolUse][0].handlers.is_empty(),
                "{handler_type} must not produce a runnable handler"
            );
            let entries = &cfg.unsupported_handlers[HookEvent::PreToolUse.as_str()];
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].matcher.as_deref(), Some("Bash"));
            assert!(entries[0].command.contains(handler_type));
        }
    }

    #[test]
    fn parses_http_handler_and_requires_url() {
        let json = r#"{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "http",
            "url": "http://localhost:8080/hooks/pre-tool-use",
            "timeout": 30,
            "headers": { "Authorization": "Bearer $MY_TOKEN" },
            "allowedEnvVars": ["MY_TOKEN"]
          }
        ]
      }
    ]
  }
}"#;
        let cfg =
            parse_hooks_value(&serde_json::from_str(json).unwrap(), Path::new("t.json")).unwrap();
        let handler = &cfg.events[&HookEvent::PreToolUse][0].handlers[0];
        let http = handler.http.as_ref().expect("http config must be parsed");
        assert_eq!(http.url, "http://localhost:8080/hooks/pre-tool-use");
        assert_eq!(
            http.headers.get("Authorization").map(String::as_str),
            Some("Bearer $MY_TOKEN")
        );
        assert_eq!(http.allowed_env_vars, vec!["MY_TOKEN"]);
        assert_eq!(handler.timeout_ms, 30_000);
        assert!(cfg.unsupported_handlers.is_empty());

        let no_url = r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"http"}]}]}}"#;
        assert!(
            parse_hooks_value(&serde_json::from_str(no_url).unwrap(), Path::new("t.json"))
                .unwrap_err()
                .contains("url")
        );
    }

    /// Non-http(s) or unparsable URLs must fail at install/parse (#3041
    /// G1.5), not silently fail the hook on every event at runtime.
    #[test]
    fn rejects_invalid_http_handler_url_at_parse_time() {
        for url in ["ftp://example.com/hook", "not a url", "file:///etc/passwd"] {
            let json = serde_json::json!({
                "hooks": { "PreToolUse": [{ "hooks": [{ "type": "http", "url": url }] }] }
            });
            let error = parse_hooks_value(&json, Path::new("t.json"))
                .expect_err("{url:?} must be rejected at parse time");
            assert!(
                error.contains("url"),
                "error should name the offending field, got: {error}"
            );
        }
    }

    #[test]
    fn rejects_zero_timeout() {
        let bad_timeout = r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"x","timeout":0}]}]}}"#;
        assert!(parse_hooks_value(
            &serde_json::from_str(bad_timeout).unwrap(),
            Path::new("t.json")
        )
        .unwrap_err()
        .contains("timeout"));
    }

    #[test]
    fn rejects_session_start_async_true() {
        let json = r#"{
  "hooks": {
    "SessionStart": [
      { "hooks": [ { "type": "command", "command": "x", "async": true } ] }
    ]
  }
}"#;
        let err = parse_hooks_value(&serde_json::from_str(json).unwrap(), Path::new("t.json"))
            .unwrap_err();
        assert!(err.contains("async"));
    }

    #[test]
    fn load_from_root_merges_claude_and_cursor_files() {
        let dir = tempfile::tempdir().unwrap();
        let hooks_dir = dir.path().join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        fs::write(hooks_dir.join("hooks.json"), superpowers_claude_json()).unwrap();
        fs::write(
            hooks_dir.join("hooks-cursor.json"),
            r#"{"hooks":{"sessionEnd":[{"command":"echo bye"}]}}"#,
        )
        .unwrap();

        let cfg = load_hooks_from_root(dir.path()).unwrap();
        assert!(cfg.events.contains_key(&HookEvent::SessionStart));
        assert!(cfg.events.contains_key(&HookEvent::SessionEnd));
    }
}
