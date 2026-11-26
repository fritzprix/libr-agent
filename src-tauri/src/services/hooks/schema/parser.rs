use super::{
    default_hook_timeout_ms, HookEvent, HookHandler, HookHttpConfig, HookMatcherGroup, HooksConfig,
    UnsupportedHookEntry,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

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

pub(super) fn parse_hooks_file(path: &Path) -> Result<HooksConfig, String> {
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

pub(super) fn parse_hooks_value(root: &Value, path: &Path) -> Result<HooksConfig, String> {
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
