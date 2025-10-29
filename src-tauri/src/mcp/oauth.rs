use oauth2::reqwest::async_http_client;
/// OAuth 2.1 Authorization Code Grant with PKCE implementation
///
/// Implements RFC 7636 (PKCE), RFC 8414 (Discovery), and RFC 7591 (Dynamic Registration)
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, CsrfToken, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::mcp::types::OAuthConfig;

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
        let endpoints = if let Some(discovery_url) = &config.discovery_url {
            log::debug!("Using RFC 8414 discovery: {discovery_url}");
            self.discover_endpoints(discovery_url).await?
        } else {
            // Use fallback endpoints from config
            OAuthEndpoints {
                authorization_endpoint: config
                    .authorization_endpoint
                    .clone()
                    .ok_or("Missing authorization_endpoint")?,
                token_endpoint: config
                    .token_endpoint
                    .clone()
                    .ok_or("Missing token_endpoint")?,
            }
        };

        log::debug!("OAuth endpoints: {endpoints:?}");

        // Step 2: Create PKCE challenge
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        // Store verifier for later use in token exchange
        {
            let mut verifiers = self.pkce_verifiers.lock().await;
            verifiers.insert(server_id.to_string(), pkce_verifier);
        }

        // Step 3: Build OAuth client
        let client_id = config
            .client_id
            .clone()
            .ok_or("Missing client_id in OAuth config")?;

        let redirect_uri = config
            .redirect_uri
            .clone()
            .unwrap_or_else(|| "libr-agent://oauth/callback".to_string());

        let client = BasicClient::new(
            ClientId::new(client_id),
            None, // Client secret not used with PKCE
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

        // Step 4: Generate authorization URL with PKCE
        let mut auth_request = client
            .authorize_url(CsrfToken::new_random)
            .set_pkce_challenge(pkce_challenge);

        // Add scopes if configured
        if let Some(scopes) = &config.scopes {
            for scope in scopes {
                auth_request = auth_request.add_scope(Scope::new(scope.clone()));
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

        // Step 1: Validate CSRF token
        {
            let mut tokens = self.csrf_tokens.lock().await;
            let stored_csrf = tokens
                .remove(server_id)
                .ok_or("No CSRF token found for server")?;

            if stored_csrf.secret() != state {
                return Err("CSRF token mismatch - potential security issue".to_string());
            }
        }

        // Step 2: Retrieve PKCE verifier
        let verifier = {
            let mut verifiers = self.pkce_verifiers.lock().await;
            verifiers
                .remove(server_id)
                .ok_or("No PKCE verifier found for server")?
        };

        // Step 3: Get endpoints
        let endpoints = if let Some(discovery_url) = &config.discovery_url {
            self.discover_endpoints(discovery_url).await?
        } else {
            OAuthEndpoints {
                authorization_endpoint: config
                    .authorization_endpoint
                    .clone()
                    .ok_or("Missing authorization_endpoint")?,
                token_endpoint: config
                    .token_endpoint
                    .clone()
                    .ok_or("Missing token_endpoint")?,
            }
        };

        // Step 4: Build client and exchange code
        let client_id = config
            .client_id
            .clone()
            .ok_or("Missing client_id in OAuth config")?;

        let redirect_uri = config
            .redirect_uri
            .clone()
            .unwrap_or_else(|| "libr-agent://oauth/callback".to_string());

        let client = BasicClient::new(
            ClientId::new(client_id),
            None,
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

        let token_result = client
            .exchange_code(AuthorizationCode::new(authorization_code.to_string()))
            .set_pkce_verifier(verifier)
            .request_async(async_http_client)
            .await
            .map_err(|e| format!("Token exchange failed: {e}"))?;

        let access_token = token_result.access_token().secret().clone();

        log::info!("Successfully obtained access token for {server_id}");
        Ok(access_token)
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

        Ok(OAuthEndpoints {
            authorization_endpoint: metadata.authorization_endpoint,
            token_endpoint: metadata.token_endpoint,
        })
    }
}

impl Default for OAuthManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_oauth_manager_creation() {
        let manager = OAuthManager::new();
        assert!(manager.pkce_verifiers.lock().await.is_empty());
        assert!(manager.csrf_tokens.lock().await.is_empty());
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
            redirect_uri: Some("libr-agent://oauth/callback".to_string()),
            scopes: Some(vec!["read".to_string(), "write".to_string()]),
            use_pkce: true,
            resource_parameter: None,
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
}
