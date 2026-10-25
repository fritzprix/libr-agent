//! Rewrite unversioned `@fre4x/*` npx package specs to `@fre4x/*@latest` so npx
//! resolves the current release instead of pinning the first cached download.

use serde_json::Value;

/// True when `command` is `npx`, `npx.cmd`, or a path whose file name is npx / npx.cmd.
pub fn is_npx_command(command: &str) -> bool {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Normalize Windows separators so `C:\...\npx.cmd` resolves on Unix hosts too.
    let normalized = trimmed.replace('\\', "/");
    let file_name = std::path::Path::new(&normalized)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(normalized.as_str());

    file_name.eq_ignore_ascii_case("npx") || file_name.eq_ignore_ascii_case("npx.cmd")
}

/// If `arg` is an unversioned `@fre4x/<pkg>` spec, return `@fre4x/<pkg>@latest`.
pub fn rewrite_unversioned_fre4x_npx_arg(arg: &str) -> Option<String> {
    const PREFIX: &str = "@fre4x/";
    if !arg.starts_with(PREFIX) {
        return None;
    }

    let package = &arg[PREFIX.len()..];
    if package.is_empty() || package.contains('/') || package.contains('@') {
        return None;
    }

    Some(format!("{PREFIX}{package}@latest"))
}

/// Rewrite args in-place when `command` is npx. Returns true if any arg changed.
pub fn rewrite_fre4x_npx_args_if_npx(command: &str, args: &mut [Value]) -> bool {
    if !is_npx_command(command) {
        return false;
    }

    let mut changed = false;
    for value in args.iter_mut() {
        let Some(arg) = value.as_str() else {
            continue;
        };
        if let Some(rewritten) = rewrite_unversioned_fre4x_npx_arg(arg) {
            *value = Value::String(rewritten);
            changed = true;
        }
    }
    changed
}

/// Migrate stored MCP server config JSON (transport.stdio args). Returns new JSON when changed.
pub fn migrate_mcp_server_config_json(config_str: &str) -> Option<String> {
    let mut config: Value = serde_json::from_str(config_str).ok()?;

    let mut changed = false;

    if let Some(transport) = config.get_mut("transport") {
        if transport.get("type").and_then(|t| t.as_str()) == Some("stdio") {
            let command = transport
                .get("command")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string();
            if let Some(args) = transport.get_mut("args").and_then(|a| a.as_array_mut()) {
                if rewrite_fre4x_npx_args_if_npx(&command, args) {
                    changed = true;
                }
            }
        }
    }

    if changed {
        serde_json::to_string(&config).ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn is_npx_command_matches_common_forms() {
        assert!(is_npx_command("npx"));
        assert!(is_npx_command("NPX"));
        assert!(is_npx_command("npx.cmd"));
        assert!(is_npx_command(r"C:\Program Files\nodejs\npx.cmd"));
        assert!(is_npx_command("/usr/local/bin/npx"));
        assert!(!is_npx_command("uvx"));
        assert!(!is_npx_command("node"));
        assert!(!is_npx_command(""));
    }

    #[test]
    fn rewrite_unversioned_fre4x_npx_arg_cases() {
        assert_eq!(
            rewrite_unversioned_fre4x_npx_arg("@fre4x/hn").as_deref(),
            Some("@fre4x/hn@latest")
        );
        assert_eq!(
            rewrite_unversioned_fre4x_npx_arg("@fre4x/yahoo-finance").as_deref(),
            Some("@fre4x/yahoo-finance@latest")
        );

        assert!(rewrite_unversioned_fre4x_npx_arg("@fre4x/hn@latest").is_none());
        assert!(rewrite_unversioned_fre4x_npx_arg("@fre4x/hn@1.2.3").is_none());
        assert!(rewrite_unversioned_fre4x_npx_arg("@fre4x/hn@beta").is_none());
        assert!(rewrite_unversioned_fre4x_npx_arg("@scope/pkg").is_none());
        assert!(rewrite_unversioned_fre4x_npx_arg("-y").is_none());
        assert!(rewrite_unversioned_fre4x_npx_arg("./local/path").is_none());
        assert!(rewrite_unversioned_fre4x_npx_arg("@fre4x/").is_none());
        assert!(rewrite_unversioned_fre4x_npx_arg("@fre4x/a/b").is_none());
    }

    #[test]
    fn rewrite_skips_non_npx_command() {
        let mut args = vec![json!("-y"), json!("@fre4x/hn")];
        assert!(!rewrite_fre4x_npx_args_if_npx("uvx", &mut args));
        assert_eq!(args[1], json!("@fre4x/hn"));
    }

    #[test]
    fn migrate_config_rewrites_stdio_npx_fre4x_args() {
        let input = json!({
            "transport": {
                "type": "stdio",
                "command": "npx",
                "args": ["-y", "@fre4x/hn"],
                "env": { "SECRET": "keep-me" }
            },
            "isActive": true
        });
        let out = migrate_mcp_server_config_json(&input.to_string()).expect("should migrate");
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(
            parsed["transport"]["args"],
            json!(["-y", "@fre4x/hn@latest"])
        );
        assert_eq!(parsed["transport"]["env"]["SECRET"], "keep-me");
        assert_eq!(parsed["isActive"], true);
    }

    #[test]
    fn migrate_config_idempotent() {
        let once = json!({
            "transport": {
                "type": "stdio",
                "command": "npx.cmd",
                "args": ["-y", "@fre4x/jupyter"]
            }
        });
        let first = migrate_mcp_server_config_json(&once.to_string())
            .expect("first pass")
            .clone();
        assert!(migrate_mcp_server_config_json(&first).is_none());
    }

    #[test]
    fn migrate_config_skips_http_and_pinned_versions() {
        let http = json!({
            "transport": {
                "type": "http",
                "url": "https://example.com/mcp"
            }
        });
        assert!(migrate_mcp_server_config_json(&http.to_string()).is_none());

        let pinned = json!({
            "transport": {
                "type": "stdio",
                "command": "npx",
                "args": ["-y", "@fre4x/hn@1.1.27"]
            }
        });
        assert!(migrate_mcp_server_config_json(&pinned.to_string()).is_none());
    }

    #[test]
    fn migrate_config_malformed_json_is_no_op() {
        assert!(migrate_mcp_server_config_json("{not json").is_none());
        assert!(migrate_mcp_server_config_json("{}").is_none());
    }
}
