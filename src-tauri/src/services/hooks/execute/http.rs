//! HTTP hook transport (#3009): header interpolation, body limits, and
//! redirect-safe request execution.

use super::handler::HookRunOutcome;
use crate::services::hooks::output::{normalize, HookOutput};
use crate::services::hooks::schema::{HookEvent, HookHttpConfig};
use std::time::Duration;
use tokio::time::timeout;

/// Interpolate `$VAR` / `${VAR}` references in HTTP hook header values
/// (#3009): names listed in `allowed_env_vars` resolve to their environment
/// value, unlisted names resolve to an empty string, and with no allowlist
/// nothing is interpolated at all.
pub(super) fn interpolate_http_header(value: &str, allowed: &[String]) -> String {
    if allowed.is_empty() {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        let brace = chars.peek() == Some(&'{');
        if brace {
            chars.next();
        }
        let mut name = String::new();
        while chars
            .peek()
            .is_some_and(|n| *n == '_' || n.is_ascii_alphanumeric())
        {
            name.push(chars.next().expect("peeked"));
        }
        let closed = !brace || chars.next() == Some('}');
        if name.is_empty() || !closed {
            out.push('$');
            if brace {
                out.push('{');
            }
            continue;
        }
        let resolved = if allowed.iter().any(|v| v == &name) {
            std::env::var(&name).unwrap_or_default()
        } else {
            String::new()
        };
        out.push_str(&resolved);
    }
    out
}

/// Hard bound on the body an `http` hook response may deliver (#3041 G1.3):
/// hook output is a small JSON decision document, and an unbounded read lets
/// a hostile or broken server grow memory without limit.
const MAX_HTTP_HOOK_BODY_BYTES: usize = 1024 * 1024;

enum HttpHookError {
    Request(reqwest::Error),
    TooLarge,
}

impl From<reqwest::Error> for HttpHookError {
    fn from(error: reqwest::Error) -> Self {
        Self::Request(error)
    }
}

/// Run a `type: "http"` hook: POST the event envelope as JSON to `url` and
/// apply the response body per the Hooks reference HTTP response handling
/// (2xx + empty body = success with no output, 2xx + JSON object = the same
/// JSON output schema as command hooks, anything else = non-blocking error)
/// (#3009).
pub(super) async fn run_http_hook(
    event: HookEvent,
    http: &HookHttpConfig,
    deadline: Duration,
    envelope_json: &str,
) -> HookRunOutcome {
    use std::str::FromStr;

    let client = match reqwest::Client::builder()
        // Never follow redirects (#3041 G1.2): a 301/302/307/308 would
        // re-POST the envelope (prompt, tool input and output) and the
        // custom headers to a host the user never approved. With redirects
        // off, a 3xx is simply a non-2xx — a non-blocking error.
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return HookRunOutcome::Skipped(format!("http hook client failed: {error}"));
        }
    };
    let mut request = client
        .post(&http.url)
        .header(reqwest::header::CONTENT_TYPE, "application/json");
    for (name, value) in &http.headers {
        match reqwest::header::HeaderName::from_str(name.as_str()) {
            Ok(header_name) => {
                let resolved = interpolate_http_header(value, &http.allowed_env_vars);
                match reqwest::header::HeaderValue::from_str(&resolved) {
                    Ok(header_value) => request = request.header(header_name, header_value),
                    Err(error) => {
                        // Silently substituting an empty string would send a
                        // request the user never approved (#3041 G1.5):
                        // skip the hook and warn instead.
                        log::warn!("http hook: invalid header value for {name:?}: {error}");
                        return HookRunOutcome::Skipped(format!(
                            "http hook: invalid header value for {name:?}: {error}"
                        ));
                    }
                }
            }
            Err(error) => {
                // Same reasoning for the name: a declared header the hook
                // cannot send means the hook does not run as approved.
                log::warn!("http hook: invalid header name {name:?}: {error}");
                return HookRunOutcome::Skipped(format!(
                    "http hook: invalid header name {name:?}: {error}"
                ));
            }
        }
    }
    let request = request.body(envelope_json.to_string());

    // One deadline covers the whole exchange — connect, send, and the body
    // read — so a server that answers headers and then stalls cannot block
    // the tool call past the handler timeout (#3041 G1.3).
    let exchange = timeout(deadline, async move {
        let response = request.send().await?;
        let status_code = response.status().as_u16();
        if !response.status().is_success() {
            return Ok((status_code, Vec::new()));
        }
        if response
            .content_length()
            .is_some_and(|len| len > MAX_HTTP_HOOK_BODY_BYTES as u64)
        {
            return Err(HttpHookError::TooLarge);
        }
        let mut response = response;
        let mut bytes: Vec<u8> = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if bytes.len() + chunk.len() > MAX_HTTP_HOOK_BODY_BYTES {
                return Err(HttpHookError::TooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok((status_code, bytes))
    })
    .await;
    let (status_code, body) = match exchange {
        Ok(Ok(exchange)) => exchange,
        Ok(Err(HttpHookError::TooLarge)) => {
            return HookRunOutcome::Skipped(format!(
                "http hook body over {MAX_HTTP_HOOK_BODY_BYTES} bytes"
            ));
        }
        Ok(Err(HttpHookError::Request(error))) => {
            return HookRunOutcome::Skipped(format!("http hook request failed: {error}"));
        }
        Err(_) => {
            return HookRunOutcome::Skipped(format!(
                "http hook timed out after {}s",
                deadline.as_secs()
            ));
        }
    };
    if !(200..300).contains(&status_code) {
        return HookRunOutcome::Skipped(format!("http hook status {status_code}"));
    }
    let body = String::from_utf8_lossy(&body);
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return HookRunOutcome::Applied(HookOutput::default());
    }
    // The body must be a JSON object using the command-hook output schema;
    // any other body (e.g. plain text) is a non-blocking error.
    let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return HookRunOutcome::Skipped("http hook body is not JSON".to_string());
    };
    if !value.is_object() {
        return HookRunOutcome::Skipped("http hook body is not a JSON object".to_string());
    }
    HookRunOutcome::Applied(normalize(trimmed, event))
}
