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
#[async_trait::async_trait]
pub trait SecretStore: Send + Sync {
    async fn get(&self, server_id: &str, name: &str) -> Result<Option<String>, String>;
    /// Store a value. Returns a notice to show the user once (the keychain fallback
    /// warning), or `None`.
    async fn set(&self, server_id: &str, name: &str, value: &str)
        -> Result<Option<String>, String>;
    async fn delete(&self, server_id: &str, name: &str) -> Result<(), String>;
    /// False when the store cannot be used at all (for example no OS keychain).
    fn available(&self) -> bool {
        true
    }
}

/// The OS keychain, with the encrypted `encrypted_secrets` fallback when it is not
/// available, in the app; an in-memory store in unit tests (CI has no keychain).
pub fn secret_store() -> &'static dyn SecretStore {
    #[cfg(not(test))]
    {
        use crate::mcp::secret_fallback::{app_key_path, FileSecretStore, LayeredSecretStore};
        static KEYCHAIN: crate::mcp::keychain::KeychainSecretStore =
            crate::mcp::keychain::KeychainSecretStore;
        static STORE: Lazy<LayeredSecretStore<'static>> = Lazy::new(|| {
            LayeredSecretStore::new(
                &KEYCHAIN,
                FileSecretStore::new(crate::state::get_mcp_server_repository(), app_key_path()),
            )
        });
        &*STORE
    }
    #[cfg(test)]
    {
        static STORE: Lazy<MemorySecretStore> = Lazy::new(MemorySecretStore::default);
        &*STORE
    }
}

#[cfg(test)]
#[derive(Default)]
pub struct MemorySecretStore {
    values: std::sync::Mutex<HashMap<(String, String), String>>,
    unavailable: bool,
}

#[cfg(test)]
impl MemorySecretStore {
    /// A store that reports itself unavailable, like a machine without an OS keychain.
    pub fn unavailable() -> Self {
        Self {
            values: Default::default(),
            unavailable: true,
        }
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl SecretStore for MemorySecretStore {
    async fn get(&self, server_id: &str, name: &str) -> Result<Option<String>, String> {
        let map = self.values.lock().map_err(|e| e.to_string())?;
        Ok(map.get(&(server_id.to_string(), name.to_string())).cloned())
    }
    async fn set(
        &self,
        server_id: &str,
        name: &str,
        value: &str,
    ) -> Result<Option<String>, String> {
        let mut map = self.values.lock().map_err(|e| e.to_string())?;
        map.insert((server_id.to_string(), name.to_string()), value.to_string());
        Ok(None)
    }
    async fn delete(&self, server_id: &str, name: &str) -> Result<(), String> {
        let mut map = self.values.lock().map_err(|e| e.to_string())?;
        map.remove(&(server_id.to_string(), name.to_string()));
        Ok(())
    }
    fn available(&self) -> bool {
        !self.unavailable
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

/// True when `value` is exactly one `{{secret:NAME}}` with a valid name and nothing else.
pub fn is_single_template(value: &str) -> bool {
    value
        .strip_prefix("{{secret:")
        .and_then(|rest| rest.strip_suffix("}}"))
        .is_some_and(|name| NAME_RE.is_match(name))
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

/// The URL as typed, with user info, every query value, a query part without `=`, and
/// the fragment shown as `***`. A value that is exactly one `{{secret:NAME}}` stays, so a
/// preview shows which secret goes where but never a value. Path segments are not masked.
pub fn mask_url_secrets(url: &str) -> String {
    let (rest, fragment) = match url.split_once('#') {
        Some((rest, _)) => (rest, "#***"),
        None => (url, ""),
    };
    let (base, query) = match rest.split_once('?') {
        Some((base, query)) => (base, Some(query)),
        None => (rest, None),
    };

    let mut masked = match base.split_once("://") {
        Some((scheme, after)) => {
            let authority_end = after.find('/').unwrap_or(after.len());
            match after[..authority_end].rsplit_once('@') {
                Some((_, host)) => format!("{scheme}://***@{host}{}", &after[authority_end..]),
                None => base.to_string(),
            }
        }
        None => base.to_string(),
    };
    if let Some(query) = query {
        let pairs = query
            .split('&')
            .map(|pair| match pair.split_once('=') {
                Some((key, value)) if !value.is_empty() && !is_single_template(value) => {
                    format!("{key}=***")
                }
                Some(_) => pair.to_string(),
                None if pair.is_empty() => String::new(),
                None => "***".to_string(),
            })
            .collect::<Vec<_>>();
        masked.push('?');
        masked.push_str(&pairs.join("&"));
    }
    masked.push_str(fragment);
    masked
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
pub async fn fill_secrets(
    store: &dyn SecretStore,
    server_id: &str,
    config: &MCPServerConfig,
) -> Result<FilledConfig, String> {
    let names = template_names(&config.transport)?;
    let mut values = HashMap::new();
    for name in &names {
        let value = store.get(server_id, name).await?.ok_or_else(|| {
            let server = config.name.as_deref().unwrap_or(server_id);
            format!(
                "MCP server '{server}' needs the secret '{name}', but no value is stored. Run tool__updateServer for '{server}' with reenterSecrets: [\"{name}\"] to enter it again."
            )
        })?;
        values.insert(name.clone(), value);
    }

    let mut filled = config.clone();
    if !values.is_empty() {
        match &mut filled.transport {
            TransportConfig::Stdio { env, .. } => {
                for value in env.values_mut() {
                    *value = fill_text(value, &values);
                }
            }
            TransportConfig::Http { url, headers, .. } => {
                *url = fill_url(url, &values);
                for value in headers.iter_mut().flat_map(|h| h.values_mut()) {
                    *value = fill_text(value, &values);
                }
            }
        }
    }
    let mut masked: Vec<String> = Vec::with_capacity(values.len() * 2);
    for value in values.into_values() {
        let encoded = encode_query_value(&value);
        if encoded != value {
            masked.push(encoded);
        }
        masked.push(value);
    }
    Ok(FilledConfig {
        config: filled,
        values: masked,
    })
}

fn fill_text(text: &str, values: &HashMap<String, String>) -> String {
    TEMPLATE_RE
        .replace_all(text, |capture: &regex::Captures| {
            values[&capture[1]].clone()
        })
        .into_owned()
}

/// Like [`fill_text`], but a value filled into the query is percent-encoded, so a `&`,
/// `#`, `+`, `%`, or space stays inside its one query parameter.
fn fill_url(url: &str, values: &HashMap<String, String>) -> String {
    let query_start = url.find('?');
    let query_end = url.find('#').unwrap_or(url.len());
    TEMPLATE_RE
        .replace_all(url, |capture: &regex::Captures| {
            let value = &values[&capture[1]];
            let at = capture.get(0).map_or(0, |m| m.start());
            if query_start.is_some_and(|start| at > start && at < query_end) {
                encode_query_value(value)
            } else {
                value.clone()
            }
        })
        .into_owned()
}

/// Percent-encode a query component. A space becomes `%20`, not `+`, so servers that
/// only percent-decode get the same value.
fn encode_query_value(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
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
    fn single_template_is_the_whole_value_with_a_valid_name() {
        assert!(is_single_template("{{secret:EXA_API_KEY}}"));
        for value in [
            "",
            "plain",
            "{{secret:exa}}",
            "{{secret:}}",
            "x{{secret:A}}",
            "{{secret:A}}x",
            "{{secret:A}}{{secret:B}}",
        ] {
            assert!(!is_single_template(value), "{value}");
        }
    }

    #[test]
    fn masked_url_keeps_templates_and_hides_every_value() {
        assert_eq!(
            mask_url_secrets("https://mcp.exa.ai/mcp?exaApiKey=abc123&mode=x"),
            "https://mcp.exa.ai/mcp?exaApiKey=***&mode=***"
        );
        assert_eq!(
            mask_url_secrets("https://mcp.exa.ai/mcp?exaApiKey={{secret:EXA_API_KEY}}&debug"),
            "https://mcp.exa.ai/mcp?exaApiKey={{secret:EXA_API_KEY}}&***"
        );
        assert_eq!(
            mask_url_secrets("https://host.example/mcp?sk-live-abc123"),
            "https://host.example/mcp?***"
        );
        assert_eq!(
            mask_url_secrets("https://user:pw@host.example/p?k=&t=v#token"),
            "https://***@host.example/p?k=&t=***#***"
        );
        assert_eq!(
            mask_url_secrets("https://host.example/mcp"),
            "https://host.example/mcp"
        );
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

    #[tokio::test]
    async fn fill_replaces_templates_and_leaves_the_source_unchanged() {
        let store = MemorySecretStore::default();
        store.set("id-1", "GITHUB_PAT", "ghp_value").await.unwrap();
        let source = github();

        let filled = fill_secrets(&store, "id-1", &source).await.expect("filled");

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

    const SPECIAL: &str = "k&y=1#a+b%2 c";

    async fn special_store(id: &str) -> MemorySecretStore {
        let store = MemorySecretStore::default();
        store.set(id, "KEY", SPECIAL).await.unwrap();
        store
    }

    #[tokio::test]
    async fn a_query_value_is_encoded_into_exactly_one_parameter() {
        let store = special_store("id-q").await;
        let source = config(serde_json::json!({
            "type": "http",
            "url": "https://mcp.exa.ai/mcp?exaApiKey={{secret:KEY}}&x=1#frag"
        }));

        let filled = fill_secrets(&store, "id-q", &source).await.expect("filled");

        let TransportConfig::Http { url, .. } = &filled.config.transport else {
            panic!("http");
        };
        let parsed = url::Url::parse(url).expect("valid url");
        let pairs: Vec<(String, String)> = parsed.query_pairs().into_owned().collect();
        assert_eq!(
            pairs,
            vec![
                ("exaApiKey".to_string(), SPECIAL.to_string()),
                ("x".to_string(), "1".to_string())
            ]
        );
        assert_eq!(parsed.fragment(), Some("frag"));
        assert!(!url.contains('+'), "{url}");
    }

    #[tokio::test]
    async fn env_header_and_non_query_url_values_are_not_encoded() {
        let store = special_store("id-raw").await;
        let http = config(serde_json::json!({
            "type": "http",
            "url": "https://example.com/{{secret:KEY}}/mcp",
            "headers": { "Authorization": "Bearer {{secret:KEY}}" }
        }));
        let stdio = config(serde_json::json!({
            "type": "stdio", "command": "npx", "env": { "TOKEN": "{{secret:KEY}}" }
        }));

        let filled = fill_secrets(&store, "id-raw", &http).await.expect("filled");
        let TransportConfig::Http { url, headers, .. } = &filled.config.transport else {
            panic!("http");
        };
        assert_eq!(url, &format!("https://example.com/{SPECIAL}/mcp"));
        assert_eq!(
            headers.as_ref().unwrap()["Authorization"],
            format!("Bearer {SPECIAL}")
        );

        let filled = fill_secrets(&store, "id-raw", &stdio)
            .await
            .expect("filled");
        let TransportConfig::Stdio { env, .. } = &filled.config.transport else {
            panic!("stdio");
        };
        assert_eq!(env["TOKEN"], SPECIAL);
    }

    #[tokio::test]
    async fn a_failed_connect_with_an_encoded_query_value_shows_no_value() {
        let store = special_store("id-err").await;
        let source = config(serde_json::json!({
            "type": "http",
            "url": "https://mcp.exa.ai/mcp?exaApiKey={{secret:KEY}}"
        }));
        let filled = fill_secrets(&store, "id-err", &source)
            .await
            .expect("filled");
        let TransportConfig::Http { url, .. } = &filled.config.transport else {
            panic!("http");
        };
        let encoded = encode_query_value(SPECIAL);
        assert!(url.contains(&encoded), "{url}");

        let client_error = format!("error sending request for url ({url})");
        let log_line = redact_url_in(&client_error, url);
        assert!(!log_line.contains(&encoded), "{log_line}");
        assert!(!log_line.contains(SPECIAL), "{log_line}");

        let server_error = format!("401: key {encoded} rejected, raw {SPECIAL}");
        let masked = filled.mask(&server_error);
        assert!(!masked.contains(&encoded), "{masked}");
        assert!(!masked.contains(SPECIAL), "{masked}");
    }

    #[tokio::test]
    async fn a_missing_value_names_the_variable_and_the_reentry_call() {
        let store = MemorySecretStore::default();
        let error = fill_secrets(&store, "id-2", &github())
            .await
            .err()
            .expect("missing value fails");
        assert!(error.contains("'GITHUB_PAT'"));
        assert!(error.contains("tool__updateServer"));
        assert!(error.contains("reenterSecrets"));
    }

    #[tokio::test]
    async fn a_config_without_templates_never_reads_the_store() {
        struct FailingStore;
        #[async_trait::async_trait]
        impl SecretStore for FailingStore {
            async fn get(&self, _: &str, _: &str) -> Result<Option<String>, String> {
                Err("keychain must not be read".to_string())
            }
            async fn set(&self, _: &str, _: &str, _: &str) -> Result<Option<String>, String> {
                unreachable!()
            }
            async fn delete(&self, _: &str, _: &str) -> Result<(), String> {
                unreachable!()
            }
        }
        let plain = config(serde_json::json!({
            "type": "stdio", "command": "npx", "env": { "TOKEN": "plain" }
        }));
        assert!(fill_secrets(&FailingStore, "id", &plain).await.is_ok());
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
