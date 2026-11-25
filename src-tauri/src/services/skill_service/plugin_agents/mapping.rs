/// Claude Code tool name → LibrAgent builtin service alias. Conservative:
/// only tools with a clear LibrAgent counterpart are mapped; everything
/// else is reported unmapped and never granted (#2923 approach 3).
///
/// Restricted Claude Code entries such as `Bash(git:*)` do not match any
/// name here and fall to "unmapped" (never granted). There is no narrower
/// read-only workspace alias in LibrAgent: `Read`/`Glob`/`Grep`/`LS` map to
/// the same `workspace` service that also carries file writes and shell,
/// and the install preview discloses every granted service as the
/// mitigation (#2923 gatekeeper G1.2).
fn map_plugin_tool(tool: &str) -> Option<&'static str> {
    Some(match tool.to_ascii_lowercase().as_str() {
        "bash" | "read" | "write" | "edit" | "multiedit" | "notebookedit" | "notebookread"
        | "glob" | "grep" | "ls" | "search" => "workspace",
        "webfetch" | "websearch" => "browser",
        "task" | "subagent" => "agent",
        "todowrite" | "plan" => "planning",
        "askuserquestion" => "ui",
        _ => return None,
    })
}

/// Split raw `tools` entries into mapped canonical aliases and unmapped
/// names. Aliases are deduplicated and sorted for deterministic configs.
pub fn map_plugin_tools(tools: &[String]) -> (Vec<String>, Vec<String>) {
    let mut mapped: Vec<String> = Vec::new();
    let mut unmapped: Vec<String> = Vec::new();
    for tool in tools {
        match map_plugin_tool(tool) {
            Some(alias) => {
                if !mapped.iter().any(|a| a == alias) {
                    mapped.push(alias.to_string());
                }
            }
            None => {
                if !unmapped.iter().any(|t| t == tool) {
                    unmapped.push(tool.clone());
                }
            }
        }
    }
    mapped.sort();
    (mapped, unmapped)
}

/// Claude Code `model` tier → LibrAgent complexity tier name, as written
/// by `read_complexity` (`low`/`normal`/`high`). `None` means "no
/// mapping": the parent session model is used (#2923 approach 4).
pub fn plugin_model_to_complexity(declared: &str) -> Option<&'static str> {
    match declared.to_ascii_lowercase().as_str() {
        "haiku" => Some("low"),
        "sonnet" => Some("normal"),
        "opus" => Some("high"),
        _ => None,
    }
}
