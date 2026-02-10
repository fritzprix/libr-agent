use serde_json::Value;
use super::super::validation;

/// Get platform-aware prompt configuration for user input
/// Returns (prompt, input_type) tuple
pub fn get_prompt_config<'a>(command: &str, args: &'a Value) -> (&'a str, &'a str) {
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

/// Build UIResource HTML for shell input form
/// Returns HTML string with embedded execution_id, prompt, and input type
pub fn build_shell_input_ui(
    execution_id: &str,
    prompt: &str,
    input_type: &str,
    nonce: &str,
) -> String {
    // Use constants to ensure tool names match definition
    use crate::mcp::builtin::workspace::tools::code_tools::{
        CANCEL_PENDING_EXECUTION, EXECUTE_PENDING_SHELL,
    };

    format!(
        r#"<!DOCTYPE html>
<html>
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <style>
      body {{
        font-family: system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
        padding: 20px;
        background: #1e1e1e;
        color: #d4d4d4;
        margin: 0;
      }}
      .container {{
        max-width: 500px;
        margin: 0 auto;
      }}
      h3 {{
        margin-top: 0;
        color: #e0e0e0;
      }}
      input {{
        width: 100%;
        padding: 10px;
        margin: 10px 0;
        background: #2d2d2d;
        color: #d4d4d4;
        border: 1px solid #444;
        border-radius: 4px;
        box-sizing: border-box;
        font-size: 14px;
      }}
      input:focus {{
        outline: none;
        border-color: #0e639c;
      }}
      button {{
        padding: 10px 20px;
        margin: 5px 5px 5px 0;
        background: #0e639c;
        color: white;
        border: none;
        border-radius: 4px;
        cursor: pointer;
        font-size: 14px;
      }}
      button:hover {{
        background: #1177bb;
      }}
      .cancel {{
        background: #6c757d;
      }}
      .cancel:hover {{
        background: #5a6268;
      }}
    </style>
  </head>
  <body>
    <div class="container">
      <h3>{}</h3>
      <form id="inputForm">
        <input
          type="{}"
          id="userInput"
          placeholder="Enter {}..."
          required
          autofocus
        />
        <div>
          <button type="submit">Submit</button>
          <button type="button" class="cancel" onclick="handleCancel()">
            Cancel
          </button>
        </div>
      </form>
    </div>

    <script>
      const executionId = '{}';
      const nonce = '{}';

      function obfuscate(input, nonce) {{
        const textEncoder = new TextEncoder();
        const inputBytes = textEncoder.encode(input);
        const nonceBytes = textEncoder.encode(nonce);
        const xored = new Uint8Array(inputBytes.length);
        for (let i = 0; i < inputBytes.length; i++) {{
          xored[i] = inputBytes[i] ^ nonceBytes[i % nonceBytes.length];
        }}
        // Convert to Base64 more safely (avoid stack overflow)
        let binary = '';
        for (let i = 0; i < xored.length; i++) {{
          binary += String.fromCharCode(xored[i]);
        }}
        return btoa(binary);
      }}

      document
        .getElementById('inputForm')
        .addEventListener('submit', async (e) => {{
          e.preventDefault();
          const userInput = document.getElementById('userInput').value;
          const obfuscatedInput = obfuscate(userInput, nonce);

          // Send to parent window (MCP Worker) - triggers 2nd tool call
          // IMPORTANT: Use window.parent.postMessage to send to parent frame
          // Using MCP-UI protocol format: type='tool' with payload wrapper
          window.parent.postMessage(
            {{
              type: 'tool',
              payload: {{
                toolName: '{}',
                params: {{
                  execution_id: executionId,
                  user_input: obfuscatedInput,
                }},
              }},
            }},
            '*',
          );

          // Clear input immediately
          document.getElementById('userInput').value = '';
          document.body.innerHTML =
            '<p style="text-align:center; color:#d4d4d4;">⏳ Executing command...</p>';
        }});

      function handleCancel() {{
        // Send to parent window (MCP Worker) - triggers cancel tool call
        // IMPORTANT: Use window.parent.postMessage to send to parent frame
        // Using MCP-UI protocol format: type='tool' with payload wrapper
        window.parent.postMessage(
          {{
            type: 'tool',
            payload: {{
              toolName: '{}',
              params: {{
                execution_id: executionId,
              }},
            }},
          }},
          '*',
        );

        document.body.innerHTML =
          '<p style="text-align:center; color:#d4d4d4;">❌ Cancelled</p>';
      }}
    </script>
  </body>
</html>"#,
        html_escape::encode_safe(prompt),
        input_type,
        input_type,
        execution_id,
        nonce,
        EXECUTE_PENDING_SHELL,
        CANCEL_PENDING_EXECUTION
    )
}
