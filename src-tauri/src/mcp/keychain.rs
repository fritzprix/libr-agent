/// Secure token storage using OS keychain
///
/// This module provides cross-platform secure storage for OAuth tokens using the system keychain:
/// - macOS: Keychain Access
/// - Windows: Credential Manager
/// - Linux: Secret Service API / libsecret
use keyring::Entry;

const SERVICE_NAME: &str = "com.libr-agent.mcp";

/// Stores an OAuth token securely in the OS keychain
///
/// # Arguments
/// * `server_id` - The unique identifier for the MCP server
/// * `token` - The OAuth access token to store
///
/// # Returns
/// A `Result` indicating success or an error string
pub async fn store_token_securely(server_id: &str, token: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE_NAME, server_id)
        .map_err(|e| format!("Failed to create keychain entry: {e}"))?;

    entry
        .set_password(token)
        .map_err(|e| format!("Failed to store token in keychain: {e}"))?;

    log::info!("Stored OAuth token securely for server: {server_id}");
    Ok(())
}

/// Retrieves an OAuth token from the OS keychain
///
/// # Arguments
/// * `server_id` - The unique identifier for the MCP server
///
/// # Returns
/// A `Result` containing `Some(token)` if found, `None` if not found, or an error
pub async fn get_cached_token(server_id: &str) -> Result<Option<String>, String> {
    let entry = Entry::new(SERVICE_NAME, server_id)
        .map_err(|e| format!("Failed to create keychain entry: {e}"))?;

    match entry.get_password() {
        Ok(token) => {
            log::debug!("Retrieved cached token for server: {server_id}");
            Ok(Some(token))
        }
        Err(keyring::Error::NoEntry) => {
            log::debug!("No cached token found for server: {server_id}");
            Ok(None)
        }
        Err(e) => Err(format!("Failed to retrieve token from keychain: {e}")),
    }
}

/// Deletes an OAuth token from the OS keychain
///
/// # Arguments
/// * `server_id` - The unique identifier for the MCP server
///
/// # Returns
/// A `Result` indicating success or an error string
pub async fn delete_token(server_id: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE_NAME, server_id)
        .map_err(|e| format!("Failed to create keychain entry: {e}"))?;

    entry
        .delete_credential()
        .map_err(|e| format!("Failed to delete token from keychain: {e}"))?;

    log::info!("Deleted OAuth token for server: {server_id}");
    Ok(())
}

/// Checks if a token exists in the keychain without retrieving it
///
/// # Arguments
/// * `server_id` - The unique identifier for the MCP server
///
/// # Returns
/// `true` if a token exists, `false` otherwise
pub async fn has_token(server_id: &str) -> bool {
    matches!(get_cached_token(server_id).await, Ok(Some(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_store_and_retrieve_token() {
        let server_id = "test-server-keychain-store";
        let token = "test-token-12345";

        // Clean up any existing token
        let _ = delete_token(server_id).await;

        // Store token
        let result = store_token_securely(server_id, token).await;
        assert!(result.is_ok());

        // Retrieve token
        let retrieved = get_cached_token(server_id).await;
        assert!(retrieved.is_ok());
        assert_eq!(retrieved.unwrap(), Some(token.to_string()));

        // Clean up
        let _ = delete_token(server_id).await;
    }

    #[tokio::test]
    async fn test_delete_token() {
        let server_id = "test-server-keychain-delete";
        let token = "test-token-67890";

        // Store token
        let _ = store_token_securely(server_id, token).await;

        // Delete token
        let result = delete_token(server_id).await;
        assert!(result.is_ok());

        // Verify deletion
        let retrieved = get_cached_token(server_id).await;
        assert!(retrieved.is_ok());
        assert_eq!(retrieved.unwrap(), None);
    }

    #[tokio::test]
    async fn test_has_token() {
        let server_id = "test-server-keychain-has";
        let token = "test-token-abcde";

        // Clean up
        let _ = delete_token(server_id).await;

        // Should not have token
        assert!(!has_token(server_id).await);

        // Store token
        let _ = store_token_securely(server_id, token).await;

        // Should have token
        assert!(has_token(server_id).await);

        // Clean up
        let _ = delete_token(server_id).await;
    }
}
