use oauth2::reqwest::async_http_client;
/// OAuth 2.1 Authorization Code Grant with PKCE implementation
///
/// Implements RFC 7636 (PKCE), RFC 8414 (Discovery), and RFC 7591 (Dynamic Registration)
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::mcp::types::OAuthConfig;

pub const DEFAULT_CALLBACK_PORT: u16 = 14207;
pub const DEFAULT_CALLBACK_URL: &str = "http://localhost:14207/callback";
/// How long the loopback listener waits for a provider callback (#3040).
pub const OAUTH_CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);

/// Stable machine-readable error prefixes (#3040 G1.6): the UI classifies
/// cancelled / timed-out / provider-error outcomes by these prefixes and never
/// by substring-matching human-readable text (a provider description such as
/// "The user cancelled the request" must not masquerade as a local cancel).
pub const AUTH_ERROR_PREFIX_CANCELLED: &str = "AUTH_CANCELLED:";
pub const AUTH_ERROR_PREFIX_TIMEOUT: &str = "AUTH_TIMEOUT:";

const ERR_CANCELLED: &str = "AUTH_CANCELLED: OAuth authorization cancelled";
const ERR_IN_PROGRESS: &str = "An OAuth authorization is already in progress";

/// One in-flight loopback authorization at a time (shared callback port).
struct ActiveOAuthFlow {
    server_id: String,
    cancel: CancellationToken,
    /// Set under the slot lock right before the token store begins. While
    /// true, `cancel_oauth_flow` is refused: a keychain write already started
    /// (it can block on an unlock prompt) cannot be undone by cancel (#3040
    /// G2.4).
    storing: bool,
}

static ACTIVE_OAUTH_FLOW: OnceLock<Mutex<Option<ActiveOAuthFlow>>> = OnceLock::new();
/// Last terminal outcome per process, kept so a card that mounts after the
/// flow ends (or misses the emit) can still read the ending (#3040 G3.1).
static LAST_OAUTH_OUTCOME: OnceLock<Mutex<Option<OAuthFlowEndedPayload>>> = OnceLock::new();

fn active_oauth_flow() -> &'static Mutex<Option<ActiveOAuthFlow>> {
    ACTIVE_OAUTH_FLOW.get_or_init(|| Mutex::new(None))
}

fn last_oauth_outcome() -> &'static Mutex<Option<OAuthFlowEndedPayload>> {
    LAST_OAUTH_OUTCOME.get_or_init(|| Mutex::new(None))
}

/// Payload for `mcp-oauth-flow-ended` and `take_oauth_flow_last_outcome` (#3040 G3.1).
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OAuthFlowEndedPayload {
    pub server_id: String,
    pub success: bool,
    /// Present on failure: the same `AUTH_*` / provider string returned by
    /// `start_oauth_flow`, so the UI reuses its prefix classifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Begin a single-flight OAuth wait. Fails if another flow already holds the port.
pub async fn begin_oauth_flow(server_id: &str) -> Result<CancellationToken, String> {
    let mut slot = active_oauth_flow().lock().await;
    if slot.is_some() {
        return Err(ERR_IN_PROGRESS.to_string());
    }
    // A new flow supersedes any previous terminal outcome for this slot.
    *last_oauth_outcome().lock().await = None;
    let cancel = CancellationToken::new();
    *slot = Some(ActiveOAuthFlow {
        server_id: server_id.to_string(),
        cancel: cancel.clone(),
        storing: false,
    });
    Ok(cancel)
}

/// Record and return the terminal outcome so every card (including ones
/// without a local promise) can show the correct ending (#3040 G3.1).
pub async fn record_oauth_flow_outcome(
    server_id: &str,
    result: &Result<String, String>,
) -> OAuthFlowEndedPayload {
    let payload = match result {
        Ok(_) => OAuthFlowEndedPayload {
            server_id: server_id.to_string(),
            success: true,
            message: None,
        },
        Err(message) => OAuthFlowEndedPayload {
            server_id: server_id.to_string(),
            success: false,
            message: Some(message.clone()),
        },
    };
    *last_oauth_outcome().lock().await = Some(payload.clone());
    payload
}

/// Take the last terminal outcome for `server_id`, if any (#3040 G3.1 / G4.1).
/// Consuming it prevents a later remount (or Disconnect → remount) from
/// replaying a stale success or error notice.
pub async fn take_oauth_flow_last_outcome(server_id: &str) -> Option<OAuthFlowEndedPayload> {
    let mut slot = last_oauth_outcome().lock().await;
    match slot.as_ref() {
        Some(outcome) if outcome.server_id == server_id => slot.take(),
        _ => None,
    }
}

/// Drop a stored terminal outcome for `server_id` (e.g. on revoke, #3040 G4.1).
pub async fn clear_oauth_flow_last_outcome(server_id: &str) {
    let mut slot = last_oauth_outcome().lock().await;
    if slot
        .as_ref()
        .is_some_and(|outcome| outcome.server_id == server_id)
    {
        *slot = None;
    }
}

/// Peek without consuming — test-only (#3068 G1.3). Production callers use
/// `take_oauth_flow_last_outcome`, which consumes the outcome.
#[cfg(test)]
pub async fn peek_oauth_flow_last_outcome(server_id: &str) -> Option<OAuthFlowEndedPayload> {
    last_oauth_outcome()
        .lock()
        .await
        .as_ref()
        .filter(|outcome| outcome.server_id == server_id)
        .cloned()
}

/// Signal cancel for the in-progress flow for `server_id` (#3040).
/// Refused once the token store has begun — see `ActiveOAuthFlow::storing`
/// (#3040 G2.4).
pub async fn cancel_oauth_flow(server_id: &str) -> Result<(), String> {
    let mut slot = active_oauth_flow().lock().await;
    match slot.as_mut() {
        Some(flow) if flow.server_id == server_id => {
            if flow.storing {
                Err("The OAuth token is being stored; this authorization can no longer be cancelled".to_string())
            } else {
                flow.cancel.cancel();
                Ok(())
            }
        }
        Some(_) => Err("No matching OAuth authorization is in progress".to_string()),
        None => Err("No OAuth authorization is in progress".to_string()),
    }
}

/// Drop the single-flight slot when the wait ends (success, fail, or cancel).
pub async fn end_oauth_flow(server_id: &str) {
    let mut slot = active_oauth_flow().lock().await;
    if slot
        .as_ref()
        .is_some_and(|flow| flow.server_id == server_id)
    {
        *slot = None;
    }
}

/// True while a single-flight OAuth flow is held for `server_id` (#3040 G1.5:
/// the UI derives the "in progress" card state from the flow owner, so a
/// remounted card shows Cancel for a flow it did not start).
pub async fn is_oauth_flow_active(server_id: &str) -> bool {
    active_oauth_flow()
        .lock()
        .await
        .as_ref()
        .is_some_and(|flow| flow.server_id == server_id)
}

/// Complete an exchanged OAuth flow: a cancel that lands during the code
/// exchange must still prevent the token from being stored (#3040 G1.4), and
/// once the store has begun the flow can no longer be cancelled (#3040 G2.4).
/// The cancel check and the `storing` state change happen under the flow-slot
/// lock, so a concurrent `cancel_oauth_flow` is either observed here or
/// refused there — it can never slip in mid-store. `store_token` is the real
/// keychain write in production and a spy in tests.
pub async fn finish_oauth_flow<F, Fut>(
    server_id: &str,
    access_token: String,
    store_token: F,
) -> Result<(), String>
where
    F: FnOnce(String, String) -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
    let mut slot = active_oauth_flow().lock().await;
    match slot.as_mut() {
        Some(flow) if flow.server_id == server_id => {
            if flow.cancel.is_cancelled() {
                return Err(ERR_CANCELLED.to_string());
            }
            flow.storing = true;
        }
        // No live slot for this server: refuse instead of storing — the
        // plain-cancel fallback would bypass the storing guard above
        // (#3040 G3.3).
        _ => {
            return Err(
                "No active OAuth authorization for this server; refusing to store the token"
                    .to_string(),
            );
        }
    }
    drop(slot);
    store_token(server_id.to_string(), access_token).await
}

/// Format a provider `error` / `error_description` callback for the UI.
pub(crate) fn provider_callback_error(error: &str, description: Option<&str>) -> String {
    match description.filter(|d| !d.is_empty()) {
        Some(desc) if error == "access_denied" => {
            format!("OAuth provider denied access: {desc}")
        }
        Some(desc) => format!("OAuth provider error ({error}): {desc}"),
        None if error == "access_denied" => "OAuth provider denied access".to_string(),
        None => format!("OAuth provider error ({error})"),
    }
}

fn is_private_or_local_host(host: &str) -> bool {
    if host.is_empty() || host == "localhost" || host == "127.0.0.1" {
        return true;
    }

    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return ip.is_loopback()
            || ip.is_unspecified()
            || ip.is_multicast()
            || match ip {
                std::net::IpAddr::V4(ipv4) => ipv4.is_private() || ipv4.is_link_local(),
                std::net::IpAddr::V6(ipv6) => {
                    let octets = ipv6.octets();
                    let first = octets[0];
                    // ULA (fc00::/7) or link-local unicast (fe80::/10)
                    first == 0xfc || first == 0xfd || (first == 0xfe && (octets[1] & 0xc0) == 0x80)
                }
            };
    }

    false
}

/// Rejects non-HTTPS OAuth endpoints and hosts on private/local networks.
fn validate_oauth_https_endpoint(url_str: &str, label: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url_str).map_err(|e| format!("Invalid {label} URL: {e}"))?;

    if parsed.scheme() != "https" {
        return Err(format!("{label} must use HTTPS scheme for security"));
    }

    let host = parsed.host_str().unwrap_or("");
    if is_private_or_local_host(host) {
        return Err(format!(
            "Security violation: Local/private {label} is not allowed"
        ));
    }

    Ok(())
}

/// Constant-time byte comparison to prevent timing attacks.
/// Note: Early return on length mismatch is acceptable here because
/// the CSRF token length is fixed and public knowledge, meaning
/// it does not leak any secret information.
fn constant_time_compare(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// Manages OAuth 2.1 flows for MCP servers
#[derive(Debug)]
pub struct OAuthManager {
    /// Stores PKCE verifiers for in-flight authorization flows
    pkce_verifiers: Arc<Mutex<HashMap<String, PkceCodeVerifier>>>,
    /// Stores CSRF tokens for security validation
    csrf_tokens: Arc<Mutex<HashMap<String, CsrfToken>>>,
}

/// OAuth authorization server metadata (RFC 8414)
#[derive(Debug, serde::Deserialize)]
struct AuthServerMetadata {
    authorization_endpoint: String,
    token_endpoint: String,
    #[allow(dead_code)] // Reserved for future dynamic client registration (RFC 7591)
    #[serde(default)]
    registration_endpoint: Option<String>,
}

/// OAuth endpoints discovered or configured
#[derive(Debug, Clone)]
pub struct OAuthEndpoints {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
}

impl OAuthManager {
    /// Creates a new OAuth manager
    pub fn new() -> Self {
        Self {
            pkce_verifiers: Arc::new(Mutex::new(HashMap::new())),
            csrf_tokens: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Starts an OAuth 2.1 authorization flow with PKCE
    ///
    /// # Arguments
    /// * `config` - OAuth configuration from MCP server config
    /// * `server_id` - Unique identifier for the MCP server
    ///
    /// # Returns
    /// The authorization URL to open in browser and CSRF state token
    pub async fn start_authorization_flow(
        &self,
        config: &OAuthConfig,
        server_id: &str,
    ) -> Result<(String, String), String> {
        log::info!("Starting OAuth flow for server: {server_id}");

        // Step 1: Discover or use configured endpoints
        let endpoints = self.resolve_endpoints(config).await?;

        log::debug!("OAuth endpoints: {endpoints:?}");

        // Step 2: Build OAuth client
        let client_id = config
            .client_id
            .clone()
            .ok_or("Missing client_id in OAuth config")?;

        let redirect_uri = config
            .redirect_uri
            .clone()
            .unwrap_or_else(|| DEFAULT_CALLBACK_URL.to_string());

        let client_secret = config.client_secret.clone().map(ClientSecret::new);

        let client = BasicClient::new(
            ClientId::new(client_id),
            client_secret,
            AuthUrl::new(endpoints.authorization_endpoint)
                .map_err(|e| format!("Invalid authorization URL: {e}"))?,
            Some(
                TokenUrl::new(endpoints.token_endpoint)
                    .map_err(|e| format!("Invalid token URL: {e}"))?,
            ),
        )
        .set_redirect_uri(
            RedirectUrl::new(redirect_uri).map_err(|e| format!("Invalid redirect URI: {e}"))?,
        );

        // Step 3: Generate authorization URL
        let mut auth_request = client.authorize_url(CsrfToken::new_random);

        if config.use_pkce {
            let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
            {
                let mut verifiers = self.pkce_verifiers.lock().await;
                verifiers.insert(server_id.to_string(), pkce_verifier);
            }
            auth_request = auth_request.set_pkce_challenge(pkce_challenge);
        }

        // Add scopes if configured using standard OAuth2 scope parameter
        if let Some(scopes) = &config.scopes {
            for scope in scopes {
                auth_request = auth_request.add_scope(Scope::new(scope.clone()));
            }
        }

        // Add any custom parameters (e.g., user_scope for Slack)
        if let Some(custom_params) = &config.custom_params {
            for (key, value) in custom_params {
                auth_request = auth_request.add_extra_param(key.clone(), value.clone());
            }
        }

        let (authorize_url, csrf_state) = auth_request.url();

        // Store CSRF token for validation
        {
            let mut tokens = self.csrf_tokens.lock().await;
            tokens.insert(server_id.to_string(), csrf_state.clone());
        }

        log::info!("Generated authorization URL for {server_id}");
        Ok((authorize_url.to_string(), csrf_state.secret().clone()))
    }

    /// Exchanges an authorization code for an access token
    ///
    /// # Arguments
    /// * `config` - OAuth configuration
    /// * `server_id` - Unique identifier for the MCP server
    /// * `authorization_code` - The code received from OAuth callback
    /// * `state` - The CSRF state token for validation
    ///
    /// # Returns
    /// The access token on success
    pub async fn exchange_code_for_token(
        &self,
        config: &OAuthConfig,
        server_id: &str,
        authorization_code: &str,
        state: &str,
    ) -> Result<String, String> {
        log::info!("Exchanging authorization code for token: {server_id}");

        // Step 1: Validate CSRF token (constant-time comparison to prevent timing attacks)
        {
            let mut tokens = self.csrf_tokens.lock().await;
            let stored_csrf = tokens
                .remove(server_id)
                .ok_or("No CSRF token found for server")?;

            if !constant_time_compare(stored_csrf.secret().as_bytes(), state.as_bytes()) {
                return Err("CSRF token mismatch - potential security issue".to_string());
            }
        }

        // Step 2: Get endpoints
        let endpoints = self.resolve_endpoints(config).await?;

        // Step 3: Build client and exchange code
        let client_id = config
            .client_id
            .clone()
            .ok_or("Missing client_id in OAuth config")?;

        let redirect_uri = config
            .redirect_uri
            .clone()
            .unwrap_or_else(|| DEFAULT_CALLBACK_URL.to_string());

        let is_slack_token_exchange =
            endpoints.token_endpoint == "https://slack.com/api/oauth.v2.user.access";

        let client_secret = config.client_secret.clone().map(ClientSecret::new);

        let mut client = BasicClient::new(
            ClientId::new(client_id),
            client_secret,
            AuthUrl::new(endpoints.authorization_endpoint)
                .map_err(|e| format!("Invalid authorization URL: {e}"))?,
            Some(
                TokenUrl::new(endpoints.token_endpoint)
                    .map_err(|e| format!("Invalid token URL: {e}"))?,
            ),
        )
        .set_redirect_uri(
            RedirectUrl::new(redirect_uri).map_err(|e| format!("Invalid redirect URI: {e}"))?,
        );

        if is_slack_token_exchange {
            client = client.set_auth_type(oauth2::AuthType::RequestBody);
        }

        let mut exchange =
            client.exchange_code(AuthorizationCode::new(authorization_code.to_string()));

        if config.use_pkce {
            // Retrieve PKCE verifier
            let verifier = {
                let mut verifiers = self.pkce_verifiers.lock().await;
                verifiers
                    .remove(server_id)
                    .ok_or("No PKCE verifier found for server")?
            };
            exchange = exchange.set_pkce_verifier(verifier);
        }

        let token_result = exchange
            .request_async(async_http_client)
            .await
            .map_err(|e| format!("Token exchange failed: {e}"))?;

        let access_token = token_result.access_token().secret().clone();

        log::info!("Successfully obtained access token for {server_id}");
        Ok(access_token)
    }

    async fn resolve_endpoints(&self, config: &OAuthConfig) -> Result<OAuthEndpoints, String> {
        if let Some(discovery_url) = &config.discovery_url {
            log::debug!("Using RFC 8414 discovery: {discovery_url}");
            self.discover_endpoints(discovery_url).await
        } else {
            let authorization_endpoint = config
                .authorization_endpoint
                .clone()
                .ok_or("Missing authorization_endpoint")?;
            let token_endpoint = config
                .token_endpoint
                .clone()
                .ok_or("Missing token_endpoint")?;
            validate_oauth_https_endpoint(&authorization_endpoint, "authorization endpoint")?;
            validate_oauth_https_endpoint(&token_endpoint, "token endpoint")?;
            Ok(OAuthEndpoints {
                authorization_endpoint,
                token_endpoint,
            })
        }
    }

    /// Discovers OAuth endpoints using RFC 8414
    ///
    /// # Arguments
    /// * `discovery_url` - The well-known discovery URL
    ///
    /// # Returns
    /// The discovered OAuth endpoints
    async fn discover_endpoints(&self, discovery_url: &str) -> Result<OAuthEndpoints, String> {
        log::debug!("Discovering OAuth endpoints from: {discovery_url}");

        validate_oauth_https_endpoint(discovery_url, "discovery URL")?;

        let client = reqwest::Client::new();
        let metadata: AuthServerMetadata = client
            .get(discovery_url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch discovery metadata: {e}"))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse discovery metadata: {e}"))?;

        log::debug!("Discovered endpoints: {metadata:?}");

        validate_oauth_https_endpoint(&metadata.authorization_endpoint, "authorization endpoint")?;
        validate_oauth_https_endpoint(&metadata.token_endpoint, "token endpoint")?;

        Ok(OAuthEndpoints {
            authorization_endpoint: metadata.authorization_endpoint,
            token_endpoint: metadata.token_endpoint,
        })
    }

    /// Parses port from a redirect URI string.
    /// E.g. "http://localhost:14207/callback" -> 14207
    /// Defaults to DEFAULT_CALLBACK_PORT if parsing fails.
    fn parse_port_from_uri(uri_str: &str) -> u16 {
        if let Ok(u) = url::Url::parse(uri_str) {
            u.port().unwrap_or(DEFAULT_CALLBACK_PORT)
        } else {
            DEFAULT_CALLBACK_PORT
        }
    }

    /// Starts a temporary localhost listener to wait for the OAuth authorization code.
    ///
    /// Ends on a valid code, a provider `error` callback, cancel, or
    /// [`OAUTH_CALLBACK_TIMEOUT`] (#3040).
    pub async fn wait_for_callback(
        &self,
        redirect_uri: &str,
        expected_state: &str,
        cancel: CancellationToken,
        timeout: Duration,
    ) -> Result<String, String> {
        use tokio::net::TcpListener;

        // Security validation: redirect_uri must be whitelisted
        let parsed_uri =
            url::Url::parse(redirect_uri).map_err(|e| format!("Invalid redirect URI: {e}"))?;

        let host = parsed_uri.host_str().unwrap_or("");
        if host != "localhost" && host != "127.0.0.1" {
            return Err(
                "Security violation: redirect_uri host must be localhost or 127.0.0.1".to_string(),
            );
        }

        if parsed_uri.scheme() != "http" {
            return Err("Security violation: redirect_uri scheme must be http".to_string());
        }

        if parsed_uri.path() != "/callback" {
            return Err("Security violation: redirect_uri path must be /callback".to_string());
        }

        let port = Self::parse_port_from_uri(redirect_uri);
        log::info!("Starting temporary OAuth callback listener on port {port}");

        let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
            .await
            .map_err(|e| format!("Failed to bind OAuth listener to port {port}: {e}"))?;

        let timeout_sleep = tokio::time::sleep(timeout);
        tokio::pin!(timeout_sleep);

        // Connections accepted but not yet read. They live inside this wait,
        // so cancel or timeout drops them together with the listener — a
        // silent connection (a browser preconnect is enough) can never hold
        // the wait hostage or keep the port bound (#3040 G3.2).
        let mut pending: futures::stream::FuturesUnordered<_> =
            futures::stream::FuturesUnordered::new();

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    return Err(ERR_CANCELLED.to_string());
                }
                _ = &mut timeout_sleep => {
                    return Err(format!(
                        "{}OAuth authorization timed out after {} minutes",
                        AUTH_ERROR_PREFIX_TIMEOUT,
                        timeout.as_secs() / 60
                    ));
                }
                Some(outcome) = futures::stream::StreamExt::next(&mut pending),
                    if !pending.is_empty() =>
                {
                    match outcome {
                        CallbackOutcome::Code(code) => return Ok(code),
                        CallbackOutcome::ProviderError(message) => {
                            return Err(message);
                        }
                        // State mismatch or junk request: respond already
                        // sent; keep listening.
                        CallbackOutcome::Ignore => {}
                    }
                }
                accept_res = listener.accept() => {
                    match accept_res {
                        Ok((stream, _)) => {
                            // Reading one connection never blocks accepting
                            // the next (#3040 G3.2); each read is raced
                            // against cancel and timeout through this same
                            // select (#3040 G2.3).
                            pending.push(handle_callback_connection(stream, expected_state));
                        }
                        Err(e) => {
                            log::error!("Error accepting connection in OAuth callback listener: {e}");
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
enum CallbackOutcome {
    Code(String),
    ProviderError(String),
    Ignore,
}

/// Read one loopback callback connection (until the request headers end,
/// bounded at 16 KB), write the browser-facing response, and classify the
/// outcome. The caller races this whole future against cancel and timeout so
/// a silent or half-open connection (a browser preconnect is enough) cannot
/// hide them or keep the port bound (#3040 G2.3).
async fn handle_callback_connection(
    mut stream: tokio::net::TcpStream,
    expected_state: &str,
) -> CallbackOutcome {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut request_bytes: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 2048];
    loop {
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                request_bytes.extend_from_slice(&chunk[..n]);
                if request_bytes.windows(4).any(|w| w == b"\r\n\r\n")
                    || request_bytes.len() >= 16_384
                {
                    break;
                }
            }
        }
    }

    let outcome = if request_bytes.is_empty() {
        None
    } else {
        let request = String::from_utf8_lossy(&request_bytes);
        parse_oauth_callback_request(&request, expected_state)
    };

    match outcome {
        Some(CallbackOutcome::Code(code)) => {
            let _ = write_http_html(
                &mut stream,
                200,
                "Authentication Successful",
                "LibrAgent has been authorized successfully. You can now close this browser tab and return to the application.",
                true,
            )
            .await;
            CallbackOutcome::Code(code)
        }
        Some(CallbackOutcome::ProviderError(message)) => {
            let _ =
                write_http_html(&mut stream, 400, "Authorization Failed", &message, false).await;
            CallbackOutcome::ProviderError(message)
        }
        Some(CallbackOutcome::Ignore) | None => {
            log::warn!("OAuth callback ignored (state mismatch or incomplete params)");
            let _ = stream
                .write_all(
                    b"HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nConnection: close\r\n\r\nFailed to authenticate. State mismatch or missing code.",
                )
                .await;
            let _ = stream.flush().await;
            CallbackOutcome::Ignore
        }
    }
}

fn parse_oauth_callback_request(request: &str, expected_state: &str) -> Option<CallbackOutcome> {
    if !request.starts_with("GET ") {
        return None;
    }
    let first_line = request.lines().next().unwrap_or("");
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let path = parts[1];
    let query_start = path.find('?')?;
    let query = &path[query_start + 1..];
    let params: HashMap<String, String> = url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();

    // State must be present and match for every callback outcome, including
    // provider errors: otherwise any local request or web page could end the
    // flow with a chosen message (#3040 G1.3).
    let state = params.get("state")?;
    if !constant_time_compare(state.as_bytes(), expected_state.as_bytes()) {
        return Some(CallbackOutcome::Ignore);
    }

    if let Some(error) = params.get("error") {
        let description = params.get("error_description").map(String::as_str);
        return Some(CallbackOutcome::ProviderError(provider_callback_error(
            error,
            description,
        )));
    }

    let code = params.get("code")?;
    Some(CallbackOutcome::Code(code.clone()))
}

/// Escapes untrusted text before it is embedded in the loopback callback page
/// (#3040 G1.2: `error` / `error_description` come from the callback URL).
fn html_escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn render_oauth_html_page(title: &str, body: &str, success: bool) -> String {
    let color = if success { "#2ea043" } else { "#f85149" };
    let title = html_escape_text(title);
    let body = html_escape_text(body);
    format!(
        "<html><head><title>LibrAgent - {title}</title>\
         <style>body{{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;text-align:center;padding-top:100px;background:#0d1117;color:#c9d1d9;}}\
         .card{{background:#161b22;padding:40px;border-radius:12px;border:1px solid #30363d;display:inline-block;max-width:400px;}}\
         h1{{color:{color};margin-top:0;}}p{{color:#8b949e;line-height:1.5;}}</style></head>\
         <body><div class='card'><h1>{title}</h1><p>{body}</p></div></body></html>"
    )
}

async fn write_http_html(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    title: &str,
    body: &str,
    success: bool,
) -> Result<(), std::io::Error> {
    use tokio::io::AsyncWriteExt;
    let status_text = if status == 200 { "OK" } else { "Bad Request" };
    let html = render_oauth_html_page(title, body, success);
    let response = format!(
        "HTTP/1.1 {status} {status_text}\r\n\
         Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\r\n\
         {html}",
        html.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await
}

impl Default for OAuthManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The single-flight slot is a process-wide static, so every test that
    /// touches it must hold this lock (tests run in parallel by default).
    fn flow_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[tokio::test]
    async fn test_oauth_manager_creation() {
        let manager = OAuthManager::new();
        assert!(manager.pkce_verifiers.lock().await.is_empty());
        assert!(manager.csrf_tokens.lock().await.is_empty());
    }

    #[test]
    fn validate_oauth_https_endpoint_rejects_private_hosts() {
        assert!(validate_oauth_https_endpoint(
            "http://auth.example.com/authorize",
            "authorization endpoint"
        )
        .is_err());
        assert!(validate_oauth_https_endpoint(
            "https://127.0.0.1/authorize",
            "authorization endpoint"
        )
        .is_err());
        assert!(
            validate_oauth_https_endpoint("https://169.254.169.254/token", "token endpoint")
                .is_err()
        );
        assert!(validate_oauth_https_endpoint(
            "https://auth.example.com/authorize",
            "authorization endpoint"
        )
        .is_ok());
    }

    #[tokio::test]
    async fn test_start_authorization_flow() {
        let manager = OAuthManager::new();
        let config = OAuthConfig {
            oauth_type: "oauth2.1".to_string(),
            discovery_url: None,
            authorization_endpoint: Some("https://auth.example.com/authorize".to_string()),
            token_endpoint: Some("https://auth.example.com/token".to_string()),
            registration_endpoint: None,
            client_id: Some("test-client".to_string()),
            client_secret: None,
            redirect_uri: Some("http://localhost:14207/callback".to_string()),
            scopes: Some(vec!["read".to_string(), "write".to_string()]),
            use_pkce: true,
            resource_parameter: None,
            custom_params: None,
        };

        let result = manager
            .start_authorization_flow(&config, "test-server")
            .await;

        assert!(result.is_ok());
        let (url, state) = result.unwrap();
        assert!(url.contains("https://auth.example.com/authorize"));
        assert!(url.contains("code_challenge"));
        assert!(url.contains("client_id=test-client"));
        assert!(!state.is_empty());

        // Verify PKCE verifier was stored
        assert!(manager
            .pkce_verifiers
            .lock()
            .await
            .contains_key("test-server"));
    }

    #[tokio::test]
    async fn test_slack_authorization_url_construction() {
        let manager = OAuthManager::new();
        let mut custom_params = HashMap::new();
        custom_params.insert(
            "user_scope".to_string(),
            "channels:read,chat:write".to_string(),
        );

        let config = OAuthConfig {
            oauth_type: "oauth2.1".to_string(),
            discovery_url: None,
            authorization_endpoint: Some("https://slack.com/oauth/v2_user/authorize".to_string()),
            token_endpoint: Some("https://slack.com/api/oauth.v2.user.access".to_string()),
            registration_endpoint: None,
            client_id: Some("test-client".to_string()),
            client_secret: None,
            redirect_uri: Some("http://localhost:14207/callback".to_string()),
            scopes: None,
            use_pkce: true,
            resource_parameter: None,
            custom_params: Some(custom_params),
        };

        let result = manager
            .start_authorization_flow(&config, "slack-server")
            .await;

        assert!(result.is_ok());
        let (url, _state) = result.unwrap();

        // Slack user oauth must contain user_scope with comma-separated scopes
        assert!(url.contains("user_scope=channels%3Aread%2Cchat%3Awrite"));
        // Slack user oauth must NOT contain standard scope parameter
        let parsed = url::Url::parse(&url).expect("valid authorization url");
        assert!(parsed.query_pairs().any(|(k, _)| k == "user_scope"));
        assert!(!parsed.query_pairs().any(|(k, _)| k == "scope"));
    }

    #[test]
    fn test_constant_time_compare() {
        assert!(constant_time_compare(b"hello", b"hello"));
        assert!(!constant_time_compare(b"hello", b"world"));
        assert!(!constant_time_compare(b"hello", b"helloo"));
    }

    #[tokio::test]
    async fn test_wait_for_callback_redirect_uri_whitelist() {
        let manager = OAuthManager::new();
        let cancel = CancellationToken::new();
        let timeout = Duration::from_secs(1);

        // Invalid host
        let res = manager
            .wait_for_callback(
                "http://malicious.example.com/callback",
                "state",
                cancel.clone(),
                timeout,
            )
            .await;
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .contains("Security violation: redirect_uri host"));

        // Invalid scheme
        let res = manager
            .wait_for_callback(
                "https://localhost/callback",
                "state",
                cancel.clone(),
                timeout,
            )
            .await;
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .contains("Security violation: redirect_uri scheme"));

        // Invalid path
        let res = manager
            .wait_for_callback("http://localhost/badpath", "state", cancel, timeout)
            .await;
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .contains("Security violation: redirect_uri path"));
    }

    #[test]
    fn provider_callback_error_formats_access_denied_and_other_errors() {
        assert_eq!(
            provider_callback_error("access_denied", Some("user said no")),
            "OAuth provider denied access: user said no"
        );
        assert_eq!(
            provider_callback_error("access_denied", None),
            "OAuth provider denied access"
        );
        assert_eq!(
            provider_callback_error("server_error", Some("down")),
            "OAuth provider error (server_error): down"
        );
    }

    #[test]
    fn parse_oauth_callback_request_reads_code_and_provider_error() {
        let ok = parse_oauth_callback_request("GET /callback?code=abc&state=s1 HTTP/1.1\r\n", "s1");
        assert!(matches!(ok, Some(CallbackOutcome::Code(code)) if code == "abc"));

        let denied = parse_oauth_callback_request(
            "GET /callback?error=access_denied&error_description=Nope&state=s1 HTTP/1.1\r\n",
            "s1",
        );
        match denied {
            Some(CallbackOutcome::ProviderError(msg)) => {
                assert!(msg.contains("denied"));
                assert!(msg.contains("Nope"));
            }
            other => panic!("expected provider error, got {other:?}"),
        }

        let mismatch =
            parse_oauth_callback_request("GET /callback?code=abc&state=wrong HTTP/1.1\r\n", "s1");
        assert!(matches!(mismatch, Some(CallbackOutcome::Ignore)));
    }

    #[test]
    fn parse_rejects_error_callbacks_without_matching_state() {
        // Missing state: not our flow, ignore entirely (#3040 G1.3).
        let no_state = parse_oauth_callback_request(
            "GET /callback?error=access_denied&error_description=Nope HTTP/1.1\r\n",
            "s1",
        );
        assert!(no_state.is_none());

        // Wrong state: same — a random local request cannot end the flow.
        let wrong_state = parse_oauth_callback_request(
            "GET /callback?error=access_denied&state=evil HTTP/1.1\r\n",
            "s1",
        );
        assert!(matches!(wrong_state, Some(CallbackOutcome::Ignore)));
    }

    #[test]
    fn render_oauth_html_page_escapes_untrusted_callback_text() {
        // #3040 G1.2: error_description from the callback URL must not reach
        // the page as raw HTML.
        let msg = provider_callback_error(
            "access_denied",
            Some("<script>alert(1)</script> & \"quoted\""),
        );
        let page = render_oauth_html_page("Authorization Failed", &msg, false);
        assert!(!page.contains("<script>"), "{page}");
        assert!(page.contains("&lt;script&gt;"), "{page}");
        assert!(page.contains("&quot;quoted&quot;"), "{page}");
    }

    #[tokio::test]
    async fn begin_oauth_flow_rejects_a_second_start() {
        let _guard = flow_test_lock().lock().await;
        let first = begin_oauth_flow("srv-a").await.expect("first start");
        assert!(is_oauth_flow_active("srv-a").await);
        assert!(!is_oauth_flow_active("srv-b").await);
        let second = begin_oauth_flow("srv-b").await;
        assert_eq!(second.unwrap_err(), ERR_IN_PROGRESS);
        first.cancel();
        end_oauth_flow("srv-a").await;
        assert!(!is_oauth_flow_active("srv-a").await);
        begin_oauth_flow("srv-b").await.expect("after end");
        end_oauth_flow("srv-b").await;
    }

    #[tokio::test]
    async fn last_outcome_survives_end_and_clears_on_next_begin() {
        let _guard = flow_test_lock().lock().await;
        let _ = begin_oauth_flow("srv-outcome").await.expect("begin");
        end_oauth_flow("srv-outcome").await;
        let ended = record_oauth_flow_outcome(
            "srv-outcome",
            &Err("AUTH_TIMEOUT: OAuth authorization timed out".to_string()),
        )
        .await;
        assert!(!ended.success);
        assert_eq!(
            peek_oauth_flow_last_outcome("srv-outcome").await,
            Some(ended.clone())
        );
        assert!(peek_oauth_flow_last_outcome("other").await.is_none());
        assert_eq!(
            take_oauth_flow_last_outcome("srv-outcome").await,
            Some(ended)
        );
        assert!(peek_oauth_flow_last_outcome("srv-outcome").await.is_none());

        let success = record_oauth_flow_outcome("srv-outcome", &Ok("ok".to_string())).await;
        assert!(success.success);
        clear_oauth_flow_last_outcome("srv-outcome").await;
        assert!(peek_oauth_flow_last_outcome("srv-outcome").await.is_none());

        let _ = begin_oauth_flow("srv-outcome").await.expect("next begin");
        let _ = record_oauth_flow_outcome("srv-outcome", &Ok("ok".to_string())).await;
        end_oauth_flow("srv-outcome").await;
        assert!(peek_oauth_flow_last_outcome("srv-outcome").await.is_some());
        let _ = begin_oauth_flow("srv-outcome")
            .await
            .expect("begin clears outcome");
        assert!(peek_oauth_flow_last_outcome("srv-outcome").await.is_none());
        end_oauth_flow("srv-outcome").await;
    }

    #[tokio::test]
    async fn cancel_oauth_flow_stops_the_wait() {
        let _guard = flow_test_lock().lock().await;
        let cancel = begin_oauth_flow("srv-cancel").await.expect("begin");
        let manager = OAuthManager::new();
        // Bind an ephemeral port so this does not fight the default app port.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("ephemeral");
        let port = listener.local_addr().expect("addr").port();
        drop(listener);
        let uri = format!("http://127.0.0.1:{port}/callback");
        let wait = tokio::spawn({
            let cancel = cancel.clone();
            async move {
                manager
                    .wait_for_callback(&uri, "state", cancel, Duration::from_secs(30))
                    .await
            }
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancel_oauth_flow("srv-cancel").await.expect("cancel");
        let err = wait.await.expect("join").expect_err("cancelled");
        assert_eq!(err, ERR_CANCELLED);
        end_oauth_flow("srv-cancel").await;
    }

    #[tokio::test]
    async fn wait_for_callback_returns_provider_deny_immediately() {
        let manager = OAuthManager::new();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("ephemeral");
        let port = listener.local_addr().expect("addr").port();
        drop(listener);
        let uri = format!("http://127.0.0.1:{port}/callback");
        let cancel = CancellationToken::new();
        let wait = tokio::spawn({
            let cancel = cancel.clone();
            let uri = uri.clone();
            async move {
                manager
                    .wait_for_callback(&uri, "state-x", cancel, Duration::from_secs(30))
                    .await
            }
        });
        tokio::time::sleep(Duration::from_millis(80)).await;
        let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .expect("connect");
        use tokio::io::AsyncWriteExt;
        stream
            .write_all(
                b"GET /callback?error=access_denied&error_description=Nope&state=state-x HTTP/1.1\r\nHost: localhost\r\n\r\n",
            )
            .await
            .expect("write");
        let err = wait.await.expect("join").expect_err("provider deny");
        assert!(err.contains("denied"), "{err}");
        assert!(err.contains("Nope"), "{err}");
    }

    /// A connection that connects and sends nothing (browser preconnect) must
    /// not hide cancel or the timeout (#3040 G2.3).
    #[tokio::test]
    async fn wait_for_callback_lets_cancel_and_timeout_pass_silent_connections() {
        let manager = OAuthManager::new();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("ephemeral");
        let port = listener.local_addr().expect("addr").port();
        drop(listener);
        let uri = format!("http://127.0.0.1:{port}/callback");

        // 1. Cancel must win while the first connection stays silent.
        let cancel = CancellationToken::new();
        let wait = tokio::spawn({
            let cancel = cancel.clone();
            let uri = uri.clone();
            async move {
                manager
                    .wait_for_callback(&uri, "state", cancel, Duration::from_secs(30))
                    .await
            }
        });
        tokio::time::sleep(Duration::from_millis(80)).await;
        let _silent = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .expect("silent connect");
        tokio::time::sleep(Duration::from_millis(80)).await;
        cancel.cancel();
        let err = tokio::time::timeout(Duration::from_secs(2), wait)
            .await
            .expect("cancel must be observed while a connection is silent")
            .expect("join")
            .expect_err("cancelled");
        assert_eq!(err, ERR_CANCELLED);

        // 2. Timeout must win on a fresh listener with only a silent connection.
        let listener2 = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("ephemeral");
        let port2 = listener2.local_addr().expect("addr").port();
        drop(listener2);
        let uri2 = format!("http://127.0.0.1:{port2}/callback");
        let manager2 = OAuthManager::new();
        let wait2 = tokio::spawn(async move {
            manager2
                .wait_for_callback(
                    &uri2,
                    "state",
                    CancellationToken::new(),
                    Duration::from_millis(150),
                )
                .await
        });
        tokio::time::sleep(Duration::from_millis(40)).await;
        let _silent2 = tokio::net::TcpStream::connect(format!("127.0.0.1:{port2}"))
            .await
            .expect("silent connect");
        let err = tokio::time::timeout(Duration::from_secs(2), wait2)
            .await
            .expect("timeout must fire while a connection is silent")
            .expect("join")
            .expect_err("timed out");
        assert!(err.starts_with(AUTH_ERROR_PREFIX_TIMEOUT), "{err}");
    }

    /// A silent connection accepted first (browser preconnect) must not
    /// delay a valid callback that arrives on a later connection: the
    /// listener keeps accepting while earlier connections are being read
    /// (#3040 G3.2).
    #[tokio::test]
    async fn wait_for_callback_reads_valid_callback_behind_silent_connection() {
        let manager = OAuthManager::new();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("ephemeral");
        let port = listener.local_addr().expect("addr").port();
        drop(listener);
        let uri = format!("http://127.0.0.1:{port}/callback");
        let wait = tokio::spawn(async move {
            manager
                .wait_for_callback(
                    &uri,
                    "state-ok",
                    CancellationToken::new(),
                    Duration::from_secs(30),
                )
                .await
        });
        tokio::time::sleep(Duration::from_millis(80)).await;

        // First connection stays silent and is never closed.
        let _silent = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .expect("silent connect");
        tokio::time::sleep(Duration::from_millis(50)).await;

        // The real callback arrives on a second connection and must be read
        // without waiting for the silent peer.
        let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .expect("callback connect");
        use tokio::io::AsyncWriteExt;
        stream
            .write_all(
                b"GET /callback?code=real-code&state=state-ok HTTP/1.1\r\nHost: localhost\r\n\r\n",
            )
            .await
            .expect("write");

        let code = tokio::time::timeout(Duration::from_secs(2), wait)
            .await
            .expect("valid callback must be read while another connection is silent")
            .expect("join")
            .expect("callback code");
        assert_eq!(code, "real-code");
    }

    #[tokio::test]
    async fn wait_for_callback_times_out() {
        let manager = OAuthManager::new();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("ephemeral");
        let port = listener.local_addr().expect("addr").port();
        drop(listener);
        let uri = format!("http://127.0.0.1:{port}/callback");
        let err = manager
            .wait_for_callback(
                &uri,
                "state",
                CancellationToken::new(),
                Duration::from_millis(80),
            )
            .await
            .expect_err("timeout");
        assert!(err.starts_with(AUTH_ERROR_PREFIX_TIMEOUT), "{err}");
        assert!(err.contains("timed out"), "{err}");
    }

    #[tokio::test]
    async fn wait_for_callback_returns_other_provider_error_immediately() {
        let manager = OAuthManager::new();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("ephemeral");
        let port = listener.local_addr().expect("addr").port();
        drop(listener);
        let uri = format!("http://127.0.0.1:{port}/callback");
        let cancel = CancellationToken::new();
        let wait = tokio::spawn({
            let cancel = cancel.clone();
            let uri = uri.clone();
            async move {
                manager
                    .wait_for_callback(&uri, "state-y", cancel, Duration::from_secs(30))
                    .await
            }
        });
        tokio::time::sleep(Duration::from_millis(80)).await;
        let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .expect("connect");
        use tokio::io::AsyncWriteExt;
        stream
            .write_all(
                b"GET /callback?error=server_error&error_description=down&state=state-y HTTP/1.1\r\nHost: localhost\r\n\r\n",
            )
            .await
            .expect("write");
        let err = wait.await.expect("join").expect_err("provider error");
        assert!(err.contains("server_error"), "{err}");
        assert!(err.contains("down"), "{err}");
    }

    /// The production flow stores the token only through `finish_oauth_flow`,
    /// which refuses to store once the flow is cancelled (#3040 G1.4, G1.7).
    ///
    /// Uses a spy store instead of the OS keychain because the keychain tests
    /// in `mcp::keychain` are `#[ignore]`d in headless CI.
    #[tokio::test]
    async fn finish_oauth_flow_refuses_to_overwrite_stored_token_after_cancel() {
        let _guard = flow_test_lock().lock().await;

        let existing = std::sync::Arc::new(std::sync::Mutex::new("pre-existing-token".to_string()));
        let store_calls = std::sync::Arc::new(std::sync::Mutex::new(0usize));

        let _cancel = begin_oauth_flow("srv-token").await.expect("begin");
        cancel_oauth_flow("srv-token").await.expect("cancel");

        let store = {
            let existing = existing.clone();
            let store_calls = store_calls.clone();
            move |_sid: String, token: String| {
                let existing = existing.clone();
                let store_calls = store_calls.clone();
                async move {
                    *store_calls.lock().expect("calls lock") += 1;
                    *existing.lock().expect("token lock") = token;
                    Ok(())
                }
            }
        };

        let err = finish_oauth_flow("srv-token", "fresh-token".to_string(), store)
            .await
            .expect_err("cancelled flow must not store");
        assert_eq!(err, ERR_CANCELLED);
        assert_eq!(*store_calls.lock().expect("calls lock"), 0);
        assert_eq!(
            existing.lock().expect("token lock").as_str(),
            "pre-existing-token",
            "existing token must survive a cancelled flow"
        );
        end_oauth_flow("srv-token").await;
    }

    /// Once the token store has begun, `cancel_oauth_flow` is refused and the
    /// cancel token stays uncancelled (#3040 G2.4).
    #[tokio::test]
    async fn cancel_oauth_flow_is_refused_while_storing() {
        let _guard = flow_test_lock().lock().await;
        let cancel = begin_oauth_flow("srv-race").await.expect("begin");

        let store = {
            let cancel_check = cancel.clone();
            move |_sid: String, _token: String| async move {
                // Simulate the keychain write blocking: cancel arrives now and
                // must be refused, not raced.
                assert!(!cancel_check.is_cancelled());
                let err = cancel_oauth_flow("srv-race")
                    .await
                    .expect_err("cancel must be refused once storing began");
                assert!(err.contains("no longer"), "{err}");
                assert!(
                    !cancel_check.is_cancelled(),
                    "refused cancel must not cancel"
                );
                Ok(())
            }
        };

        finish_oauth_flow("srv-race", "fresh-token".to_string(), store)
            .await
            .expect("uncancelled flow stores");
        assert!(!cancel.is_cancelled());
        end_oauth_flow("srv-race").await;
    }

    #[tokio::test]
    async fn finish_oauth_flow_stores_token_when_not_cancelled() {
        let _guard = flow_test_lock().lock().await;

        let stored = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
        let _cancel = begin_oauth_flow("srv-ok").await.expect("begin");

        let store = {
            let stored = stored.clone();
            move |sid: String, token: String| {
                let stored = stored.clone();
                async move {
                    *stored.lock().expect("stored lock") = Some(format!("{sid}:{token}"));
                    Ok(())
                }
            }
        };

        finish_oauth_flow("srv-ok", "fresh-token".to_string(), store)
            .await
            .expect("uncancelled flow stores");
        assert_eq!(
            stored.lock().expect("stored lock").as_deref(),
            Some("srv-ok:fresh-token")
        );
        end_oauth_flow("srv-ok").await;
    }
}
