//! `{{secret:NAME}}` templates in MCP server transports (#2548).
//!
//! The stored config always keeps the template. The values live in a [`SecretStore`]
//! and are filled in only for the process or request that starts the server.

use crate::mcp::types::{MCPServerConfig, TransportConfig};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::{BTreeSet, HashMap};

static TEMPLATE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\{\{secret:([^{}]*)\}\}").expect("valid template regex"));
static NAME_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[A-Z][A-Z0-9_]*$").expect("valid name regex"));

/// Where secret values are kept, per server id and secret name.
pub trait SecretStore: Send + Sync {
    fn get(&self, server_id: &str, name: &str) -> Result<Option<String>, String>;
    fn set(&self, server_id: &str, name: &str, value: &str) -> Result<(), String>;
    fn delete(&self, server_id: &str, name: &str) -> Result<(), String>;
}

/// The OS keychain in the app; an in-memory store in unit tests (CI has no keychain).
pub fn secret_store() -> &'static dyn SecretStore {
    #[cfg(not(test))]
    {
        static STORE: crate::mcp::keychain::KeychainSecretStore =
            crate::mcp::keychain::KeychainSecretStore;
        &STORE
    }
    #[cfg(test)]
    {
        static STORE: Lazy<MemorySecretStore> = Lazy::new(MemorySecretStore::default);
        &*STORE
    }
}

#[cfg(test)]
#[derive(Default)]
pub struct MemorySecretStore(std::sync::Mutex<HashMap<(String, String), String>>);

#[cfg(test)]
impl SecretStore for MemorySecretStore {
    fn get(&self, server_id: &str, name: &str) -> Result<Option<String>, String> {
        let map = self.0.lock().map_err(|e| e.to_string())?;
        Ok(map.get(&(server_id.to_string(), name.to_string())).cloned())
    }
    fn set(&self, server_id: &str, name: &str, value: &str) -> Result<(), String> {
        let mut map = self.0.lock().map_err(|e| e.to_string())?;
        map.insert((server_id.to_string(), name.to_string()), value.to_string());
        Ok(())
    }
    fn delete(&self, server_id: &str, name: &str) -> Result<(), String> {
        let mut map = self.0.lock().map_err(|e| e.to_string())?;
        map.remove(&(server_id.to_string(), name.to_string()));
        Ok(())
    }
}

/// The strings that may hold a template: env values, header values, and the URL.
fn template_slots(transport: &TransportConfig) -> Vec<&String> {
    match transport {
        TransportConfig::Stdio { env, .. } => env.values().collect(),
        TransportConfig::Http { url, headers, .. } => std::iter::once(url)
            .chain(headers.iter().flat_map(|h| h.values()))
            .collect(),
    }
}

fn template_slots_mut(transport: &mut TransportConfig) -> Vec<&mut String> {
    match transport {
        TransportConfig::Stdio { env, .. } => env.values_mut().collect(),
        TransportConfig::Http { url, headers, .. } => std::iter::once(url)
            .chain(headers.iter_mut().flat_map(|h| h.values_mut()))
            .collect(),
    }
}

/// The secret names the transport uses. Fails on a name that is not `^[A-Z][A-Z0-9_]*$`.
pub fn template_names(transport: &TransportConfig) -> Result<BTreeSet<String>, String> {
    let mut names = BTreeSet::new();
    for slot in template_slots(transport) {
        for capture in TEMPLATE_RE.captures_iter(slot) {
            let name = &capture[1];
            if !NAME_RE.is_match(name) {
                return Err(format!(
                    "Invalid secret template '{{{{secret:{name}}}}}': the name must match ^[A-Z][A-Z0-9_]*$ (for example GITHUB_PAT)"
                ));
            }
            names.insert(name.to_string());
        }
    }
    Ok(names)
}

/// Parse the `secret_names` column. A missing or broken value means no stored names.
pub fn parse_secret_names(column: Option<&str>) -> BTreeSet<String> {
    column
        .and_then(|json| serde_json::from_str::<BTreeSet<String>>(json).ok())
        .unwrap_or_default()
}

/// Value for the `secret_names` column: a JSON array of names, or NULL when empty.
pub fn secret_names_column(names: &BTreeSet<String>) -> Option<String> {
    if names.is_empty() {
        return None;
    }
    serde_json::to_string(names).ok()
}

/// A config with its secret values filled in. Only the starting process or request may
/// see it; never store or log it.
pub struct FilledConfig {
    pub config: MCPServerConfig,
    values: Vec<String>,
}

impl FilledConfig {
    /// Replace every filled value in `text` (for example a server error) with `***`.
    pub fn mask(&self, text: &str) -> String {
        mask_values(text, self.values.iter().map(String::as_str))
    }

    /// The filled config and the values to mask in its errors.
    pub fn into_parts(self) -> (MCPServerConfig, Vec<String>) {
        (self.config, self.values)
    }
}

/// The URL without user info, query, and fragment, so a log line never shows a value
/// filled into it.
pub fn redact_url(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut parsed) => {
            let _ = parsed.set_username("");
            let _ = parsed.set_password(None);
            parsed.set_query(None);
            parsed.set_fragment(None);
            parsed.to_string()
        }
        Err(_) => "<invalid URL>".to_string(),
    }
}

/// Replace `url` in `text` (as given, and as the URL parser prints it, which is how
/// HTTP client errors show it) with [`redact_url`].
pub fn redact_url_in(text: &str, url: &str) -> String {
    let redacted = redact_url(url);
    let normalized = url::Url::parse(url).map(|parsed| parsed.to_string());
    let text = text.replace(url, &redacted);
    match normalized {
        Ok(normalized) if normalized != url => text.replace(&normalized, &redacted),
        _ => text,
    }
}

pub fn mask_values<'a>(text: &str, values: impl IntoIterator<Item = &'a str>) -> String {
    values
        .into_iter()
        .filter(|value| !value.is_empty())
        .fold(text.to_string(), |masked, value| {
            masked.replace(value, "***")
        })
}

/// Replace every `{{secret:NAME}}` with the stored value of server `server_id`.
///
/// A config without templates is returned unchanged and the store is not read.
pub fn fill_secrets(
    store: &dyn SecretStore,
    server_id: &str,
    config: &MCPServerConfig,
) -> Result<FilledConfig, String> {
    let names = template_names(&config.transport)?;
    let mut values = HashMap::new();
    for name in &names {
        let value = store.get(server_id, name)?.ok_or_else(|| {
            let server = config.name.as_deref().unwrap_or(server_id);
            format!(
                "MCP server '{server}' needs the secret '{name}', but no value is stored. Run tool__updateServer for '{server}' with reenterSecrets: [\"{name}\"] to enter it again."
            )
        })?;
        values.insert(name.clone(), value);
    }

    let mut filled = config.clone();
    if !values.is_empty() {
        for slot in template_slots_mut(&mut filled.transport) {
            let replaced = TEMPLATE_RE.replace_all(slot, |capture: &regex::Captures| {
                values[&capture[1]].clone()
            });
            *slot = replaced.into_owned();
        }
    }
    Ok(FilledConfig {
        config: filled,
        values: values.into_values().collect(),
    })
}

/// Whether an update keeps the same target, so stored values may be reused:
/// same URL scheme, host, and port, or the same `command` and `args`.
pub fn same_target(old: &TransportConfig, new: &TransportConfig) -> bool {
    match (old, new) {
        (
            TransportConfig::Stdio {
                command: old_command,
                args: old_args,
                ..
            },
            TransportConfig::Stdio {
                command: new_command,
                args: new_args,
                ..
            },
        ) => old_command == new_command && old_args == new_args,
        (
            TransportConfig::Http { url: old_url, .. },
            TransportConfig::Http { url: new_url, .. },
        ) => match (url::Url::parse(old_url), url::Url::parse(new_url)) {
            (Ok(old), Ok(new)) => {
                old.scheme() == new.scheme()
                    && old.host_str().is_some()
                    && old.host_str() == new.host_str()
                    && old.port_or_known_default() == new.port_or_known_default()
            }
            _ => false,
        },
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(transport: serde_json::Value) -> MCPServerConfig {
        serde_json::from_value(serde_json::json!({ "name": "srv", "transport": transport }))
            .expect("config")
    }

    fn github() -> MCPServerConfig {
        config(serde_json::json!({
            "type": "http",
            "url": "https://api.githubcopilot.com/mcp/",
            "headers": { "Authorization": "Bearer {{secret:GITHUB_PAT}}" }
        }))
    }

    #[test]
    fn names_come_from_env_headers_and_url() {
        let exa = config(serde_json::json!({
            "type": "http",
            "url": "https://mcp.exa.ai/mcp?exaApiKey={{secret:EXA_API_KEY}}"
        }));
        let stdio = config(serde_json::json!({
            "type": "stdio", "command": "npx", "args": ["{{secret:NOT_A_SLOT}}"],
            "env": { "A": "{{secret:TOKEN_A}}", "B": "x-{{secret:TOKEN_B}}-y" }
        }));
        assert_eq!(
            template_names(&exa.transport).unwrap(),
            BTreeSet::from(["EXA_API_KEY".to_string()])
        );
        assert_eq!(
            template_names(&github().transport).unwrap(),
            BTreeSet::from(["GITHUB_PAT".to_string()])
        );
        assert_eq!(
            template_names(&stdio.transport).unwrap(),
            BTreeSet::from(["TOKEN_A".to_string(), "TOKEN_B".to_string()])
        );
    }

    #[test]
    fn a_lowercase_or_empty_name_is_rejected() {
        for bad in ["exaApiKey", "", "1KEY", "KEY-1"] {
            let cfg = config(serde_json::json!({
                "type": "stdio", "command": "x", "env": { "K": format!("{{{{secret:{bad}}}}}") }
            }));
            assert!(template_names(&cfg.transport).is_err(), "{bad}");
        }
    }

    #[test]
    fn fill_replaces_templates_and_leaves_the_source_unchanged() {
        let store = MemorySecretStore::default();
        store.set("id-1", "GITHUB_PAT", "ghp_value").unwrap();
        let source = github();

        let filled = fill_secrets(&store, "id-1", &source).expect("filled");

        let TransportConfig::Http { headers, .. } = &filled.config.transport else {
            panic!("http");
        };
        assert_eq!(
            headers.as_ref().unwrap()["Authorization"],
            "Bearer ghp_value"
        );
        let stored = serde_json::to_string(&source).unwrap();
        assert!(stored.contains("{{secret:GITHUB_PAT}}"));
        assert!(!stored.contains("ghp_value"));
        assert_eq!(filled.mask("401 for token ghp_value"), "401 for token ***");
    }

    #[test]
    fn a_missing_value_names_the_variable_and_the_reentry_call() {
        let store = MemorySecretStore::default();
        let error = fill_secrets(&store, "id-2", &github())
            .err()
            .expect("missing value fails");
        assert!(error.contains("'GITHUB_PAT'"));
        assert!(error.contains("tool__updateServer"));
        assert!(error.contains("reenterSecrets"));
    }

    #[test]
    fn a_config_without_templates_never_reads_the_store() {
        struct FailingStore;
        impl SecretStore for FailingStore {
            fn get(&self, _: &str, _: &str) -> Result<Option<String>, String> {
                Err("keychain must not be read".to_string())
            }
            fn set(&self, _: &str, _: &str, _: &str) -> Result<(), String> {
                unreachable!()
            }
            fn delete(&self, _: &str, _: &str) -> Result<(), String> {
                unreachable!()
            }
        }
        let plain = config(serde_json::json!({
            "type": "stdio", "command": "npx", "env": { "TOKEN": "plain" }
        }));
        assert!(fill_secrets(&FailingStore, "id", &plain).is_ok());
    }

    #[test]
    fn reuse_needs_the_same_scheme_host_and_port_or_command_and_args() {
        let http = |url: &str| config(serde_json::json!({ "type": "http", "url": url })).transport;
        let stdio = |cmd: &str, arg: &str| {
            config(serde_json::json!({ "type": "stdio", "command": cmd, "args": [arg] })).transport
        };
        assert!(same_target(
            &http("https://mcp.exa.ai/mcp?k=1"),
            &http("https://mcp.exa.ai:443/other")
        ));
        assert!(!same_target(
            &http("https://a.example/mcp"),
            &http("http://a.example/mcp")
        ));
        assert!(!same_target(
            &http("https://a.example/mcp"),
            &http("https://b.example/mcp")
        ));
        assert!(!same_target(
            &http("https://a.example/mcp"),
            &http("https://a.example:8443/mcp")
        ));
        assert!(same_target(&stdio("npx", "srv"), &stdio("npx", "srv")));
        assert!(!same_target(&stdio("npx", "srv"), &stdio("uvx", "srv")));
        assert!(!same_target(&stdio("npx", "srv"), &stdio("npx", "other")));
        assert!(!same_target(
            &stdio("npx", "srv"),
            &http("https://a.example")
        ));
    }

    #[test]
    fn the_names_column_round_trips_and_is_null_when_empty() {
        let names = BTreeSet::from(["B".to_string(), "A".to_string()]);
        let column = secret_names_column(&names);
        assert_eq!(column.as_deref(), Some(r#"["A","B"]"#));
        assert_eq!(parse_secret_names(column.as_deref()), names);
        assert_eq!(secret_names_column(&BTreeSet::new()), None);
        assert!(parse_secret_names(None).is_empty());
    }
}
