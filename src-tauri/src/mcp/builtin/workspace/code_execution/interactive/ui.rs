use crate::mcp::builtin::workspace::tools::code_tools::{
    CANCEL_PENDING_EXECUTION, EXECUTE_PENDING_SHELL,
};

/// Build UIResource HTML for shell input form
/// Returns HTML string with embedded execution_id, prompt, and input type
pub fn build_shell_input_ui(
    execution_id: &str,
    prompt: &str,
    input_type: &str,
    nonce: &str,
) -> String {
    let safe_input_type = if input_type.eq_ignore_ascii_case("password") {
        "password"
    } else {
        "text"
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
          placeholder="Enter input..."
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
                  executionId: executionId,
                  userInput: obfuscatedInput,
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
                executionId: executionId,
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
        safe_input_type,
        execution_id,
        nonce,
        EXECUTE_PENDING_SHELL,
        CANCEL_PENDING_EXECUTION
    )
}

#[cfg(test)]
mod tests {
    use super::build_shell_input_ui;

    #[test]
    fn test_build_shell_input_ui_whitelists_input_type() {
        let html = build_shell_input_ui("exec-1", "Prompt", "password", "nonce-1");
        assert!(
            html.contains(r#"type="password""#),
            "password should be preserved as allowed input type"
        );

        let html_with_invalid_type = build_shell_input_ui(
            "exec-2",
            "Prompt",
            r#"text" autofocus onfocus="alert(1)""#,
            "nonce-2",
        );
        assert!(
            html_with_invalid_type.contains(r#"type="text""#),
            "invalid input_type should be downgraded to text"
        );
        assert!(
            !html_with_invalid_type.contains("autofocus onfocus"),
            "injected attributes must not appear in generated HTML"
        );
    }

    #[test]
    fn test_build_shell_input_ui_uses_static_placeholder() {
        let html = build_shell_input_ui("exec-3", "Prompt", "password", "nonce-3");
        assert!(html.contains(r#"placeholder="Enter input...""#));
        assert!(!html.contains("Enter password..."));
    }
}
