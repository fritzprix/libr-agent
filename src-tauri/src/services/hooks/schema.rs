use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const DEFAULT_HOOK_TIMEOUT_SECS: u64 = 60;

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
    pub powershell: Option<String>,
    pub timeout_secs: u64,
    pub r#async: bool,
    pub shell: Option<String>,
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
}

pub fn load_hooks_from_root(root: &Path) -> Result<HooksConfig, String> {
    let hooks_dir = root.join("hooks");
    let claude_path = hooks_dir.join("hooks.json");
    let cursor_path = hooks_dir.join("hooks-cursor.json");

    let mut merged = HooksConfig {
        version: None,
        events: BTreeMap::new(),
        unsupported_events: BTreeMap::new(),
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
    Ok(())
}

fn parse_hooks_file(path: &Path) -> Result<HooksConfig, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let root: Value =
        serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))?;
    parse_hooks_value(&root, path)
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

    for (raw_key, groups_value) in hooks_obj {
        match normalize_event_key(raw_key) {
            Ok(event) => {
                let groups = parse_matcher_groups(groups_value, event, path)?;
                events.insert(event, groups);
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
) -> Result<Vec<HookMatcherGroup>, String> {
    let arr = value
        .as_array()
        .ok_or_else(|| format!("{}: {} must be an array", path.display(), event.as_str()))?;

    let mut groups = Vec::with_capacity(arr.len());
    for (idx, entry) in arr.iter().enumerate() {
        groups.push(parse_matcher_group(entry, event, path, idx)?);
    }
    Ok(groups)
}

fn parse_matcher_group(
    value: &Value,
    event: HookEvent,
    path: &Path,
    idx: usize,
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

    let handlers = if let Some(nested) = obj.get("hooks") {
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

    Ok(HookMatcherGroup { matcher, handlers })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawHandler {
    #[serde(rename = "type")]
    handler_type: Option<String>,
    command: Option<String>,
    powershell: Option<String>,
    timeout: Option<u64>,
    timeout_ms: Option<u64>,
    r#async: Option<bool>,
    shell: Option<String>,
}

fn parse_handler(
    value: &Value,
    event: HookEvent,
    label: &str,
    path: &Path,
) -> Result<HookHandler, String> {
    let raw: RawHandler = serde_json::from_value(value.clone())
        .map_err(|e| format!("{}: invalid handler at {label}: {e}", path.display()))?;

    let handler_type = raw.handler_type.unwrap_or_else(|| "command".to_string());
    if handler_type != "command" {
        return Err(format!(
            "{}: {label}: handler type must be \"command\", got \"{handler_type}\"",
            path.display()
        ));
    }

    let command = raw
        .command
        .ok_or_else(|| format!("{}: {label}: \"command\" is required", path.display()))?;

    let timeout_secs = resolve_timeout(raw.timeout, raw.timeout_ms)
        .map_err(|e| format!("{}: {label}: {e}", path.display()))?;

    let async_flag = raw.r#async.unwrap_or(false);
    if event == HookEvent::SessionStart && async_flag {
        return Err(format!(
            "{}: {label}: SessionStart hooks cannot use async: true",
            path.display()
        ));
    }

    Ok(HookHandler {
        command,
        powershell: raw.powershell,
        timeout_secs,
        r#async: async_flag,
        shell: raw.shell,
    })
}

fn resolve_timeout(timeout: Option<u64>, timeout_ms: Option<u64>) -> Result<u64, String> {
    if let Some(secs) = timeout {
        if secs == 0 {
            return Err("timeout must be greater than 0".to_string());
        }
        return Ok(secs);
    }
    if let Some(ms) = timeout_ms {
        if ms == 0 {
            return Err("timeoutMs must be greater than 0".to_string());
        }
        let secs = ms.div_ceil(1000).max(1);
        return Ok(secs);
    }
    Ok(DEFAULT_HOOK_TIMEOUT_SECS)
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
        assert_eq!(h.timeout_secs, DEFAULT_HOOK_TIMEOUT_SECS);
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
    fn timeout_ms_alias_rounds_up_to_seconds() {
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
            cfg.events[&HookEvent::SessionStart][0].handlers[0].timeout_secs,
            2
        );
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
    fn rejects_non_command_type_and_zero_timeout() {
        let bad_type =
            r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"prompt","command":"x"}]}]}}"#;
        assert!(parse_hooks_value(
            &serde_json::from_str(bad_type).unwrap(),
            Path::new("t.json")
        )
        .unwrap_err()
        .contains("command"));

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
