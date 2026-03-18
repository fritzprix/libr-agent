use serde_json::Value;

use crate::mcp::builtin::workspace::{
    PendingShellExecution, WorkspaceServer, PERSISTENT_SHELL_TOOL,
};
use crate::mcp::types::MCPResult;

// Import validation and ui from sibling modules
use super::super::validation;
use super::ui;

impl WorkspaceServer {
    /// Handle interactive shell execution (1st tool call)
    /// Returns UIResource with execution_id for user input
    pub(crate) async fn handle_interactive_shell(
        &self,
        command: &str,
        args: &Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        use crate::mcp::builtin::workspace::utils::sanitize_command_for_logging;

        let execution_id = uuid::Uuid::new_v4().to_string();
        let session_id = session_id.to_string();

        // Sanitize command for storage/logging
        let sanitized_command = sanitize_command_for_logging(command);

        // Extract run_mode from 1st call (will be used in 2nd call)
        let run_mode = args
            .get("run_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("sync")
            .to_string();

        // Generate nonce for client-side obfuscation
        // SECURITY WARNING:
        // XOR-based obfuscation with a UUID nonce provides only limited security.
        // Since the nonce is transmitted in the HTML, an attacker who can intercept or observe
        // the HTML content can easily reverse the obfuscation by applying the same XOR operation.
        // This approach protects against casual logging but NOT against determined attackers.
        // If the threat model requires protection against active attackers who can observe the UI content,
        // consider using a stronger encryption method (e.g., AES with secure key exchange via Web Crypto API).
        let encryption_nonce = uuid::Uuid::new_v4().to_string();

        // Store pending execution
        let pending = PendingShellExecution {
            execution_id: execution_id.clone(),
            session_id,
            executable_command: command.to_string(), // Will be executed (may get -S flag)
            display_command: sanitized_command.clone(), // For logs/UI
            run_mode,                                // Store for 2nd call
            timeout: args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(30), // Command execution timeout
            encryption_nonce: encryption_nonce.clone(),
            created_at: chrono::Utc::now(),
        };

        self.pending_executions.insert(pending);

        // Build UIResource with platform-aware prompt
        let (prompt, input_type) = self.get_prompt_config(command, args);
        let html = ui::build_shell_input_ui(&execution_id, prompt, input_type, &encryption_nonce);

        // Create UI resource JSON
        let _ui_resource = serde_json::json!({
            "uri": format!("ui://shell-input/{}", execution_id),
            "mimeType": "application/json",
            "text": html,
            "_meta": {
                "title": "Shell Command Input",
                "execution_id": execution_id,
                "created_at": chrono::Utc::now().to_rfc3339()
            }
        });

        // Return response with text and resource
        Ok(crate::mcp::builtin::utils::create_resource_response(
            &format!("ui://shell-input/{}", execution_id),
            "application/json",
            &html,
            "workspace",
            PERSISTENT_SHELL_TOOL,
            Some(&format!(
                "⏳ Waiting for user input\nExecution ID: {execution_id}\nCommand: {sanitized_command}"
            )),
        ))
    }

    /// Get platform-aware prompt configuration for user input
    /// Returns (prompt, input_type) tuple
    fn get_prompt_config<'a>(&self, command: &str, args: &'a Value) -> (&'a str, &'a str) {
        // Check if privilege escalation detected (Unix only)
        let is_privilege_cmd = validation::detect_privilege_escalation(command);

        if is_privilege_cmd {
            ("Enter your sudo password:", "password")
        } else {
            // Use custom prompt from args
            let prompt = args
                .get("input_prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("Enter input:");
            let input_type = args
                .get("input_type")
                .and_then(|v| v.as_str())
                .unwrap_or("text");
            (prompt, input_type)
        }
    }
}
