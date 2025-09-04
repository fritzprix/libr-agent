use serde_json::{json, Value};

pub fn success_response_with_text_and_resource(
    request_id: Value,
    message: &str,
    ui_resource: Value,
) -> crate::mcp::MCPResponse {
    crate::mcp::MCPResponse::success(
        request_id,
        json!({
            "content": [
                {
                    "type": "text",
                    "text": message
                },
                ui_resource
            ]
        }),
    )
}

pub fn create_export_ui_resource(
    request_id: u64,
    title: &str,
    files: &[String],
    export_type: &str,
    download_path: &str,
    content: String,
) -> Value {
    json!({
        "type": "resource",
        "resource": {
            "uri": format!("ui://export/{}/{}", export_type.to_lowercase(), request_id),
            "mimeType": "text/html",
            "text": content,
            "title": title,
            "annotations": {
                "export_type": export_type,
                "file_count": files.len(),
                "download_path": download_path,
                "created_at": chrono::Utc::now().to_rfc3339()
            }
        }
    })
}

pub fn create_html_export_ui(
    title: &str,
    files: &[String],
    export_type: &str,
    download_path: &str,
    _display_name: &str,
) -> String {
    let files_list = files
        .iter()
        .map(|f| format!("<li class='file-item'>{}</li>", html_escape::encode_text(f)))
        .collect::<Vec<_>>()
        .join("");

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            max-width: 600px;
            margin: 0 auto;
            padding: 20px;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            min-height: 100vh;
        }}
        .container {{
            background: rgba(255, 255, 255, 0.1);
            border-radius: 15px;
            padding: 30px;
            backdrop-filter: blur(10px);
            box-shadow: 0 8px 32px 0 rgba(31, 38, 135, 0.37);
        }}
        h1 {{
            text-align: center;
            margin-bottom: 30px;
            font-size: 2em;
            text-shadow: 2px 2px 4px rgba(0,0,0,0.3);
        }}
        .export-info {{
            background: rgba(255, 255, 255, 0.1);
            border-radius: 10px;
            padding: 20px;
            margin-bottom: 30px;
        }}
        .download-btn {{
            background: linear-gradient(45deg, #2196F3, #21CBF3);
            color: white;
            border: none;
            padding: 15px 30px;
            font-size: 18px;
            border-radius: 25px;
            cursor: pointer;
            transition: all 0.3s ease;
            box-shadow: 0 4px 15px 0 rgba(33, 150, 243, 0.3);
        }}
        .download-btn:hover {{
            transform: translateY(-2px);
            box-shadow: 0 6px 20px 0 rgba(33, 150, 243, 0.5);
        }}
        .download-btn:disabled {{
            opacity: 0.6;
            cursor: not-allowed;
            transform: none;
        }}
        .status-message {{
            margin-top: 15px;
            padding: 10px;
            border-radius: 5px;
            text-align: center;
        }}
        .success {{ background-color: rgba(76, 175, 80, 0.3); }}
        .error {{ background-color: rgba(244, 67, 54, 0.3); }}
        .loading {{ background-color: rgba(255, 193, 7, 0.3); }}
        ul {{ padding: 0; list-style: none; }}
        .file-item {{
            background: rgba(255, 255, 255, 0.1);
            margin: 5px 0;
            padding: 8px 12px;
            border-radius: 5px;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>🎉 {}</h1>
        <div class="export-info">
            <h3>📦 Export Type: {}</h3>
            <p>📅 Created: {}</p>
            <p>📁 Files: {} items</p>
            <h4>📋 Included Files:</h4>
            <ul>{}</ul>
        </div>
        <div style="text-align: center;">
            <button id="downloadBtn" onclick="downloadFile()" class="download-btn">
                ⬇️ Download Now
            </button>
            <div id="statusMessage" class="status-message" style="display: none;"></div>
        </div>
    </div>

    <script>
        const downloadBtn = document.getElementById('downloadBtn');
        const statusMessage = document.getElementById('statusMessage');

        async function downloadFile() {{
            downloadBtn.disabled = true;
            downloadBtn.textContent = '⏳ Downloading...';
            showStatus('Preparing download...', 'loading');

            try {{
                // MCP-UI 표준 Tool Call Action으로 다운로드 요청
                window.parent.postMessage({{
                    type: 'tool',
                    payload: {{
                        toolName: 'download_workspace_file',
                        params: {{
                            filePath: '{}'
                        }}
                    }}
                }}, '*');

                showStatus('Download request sent!', 'success');
            }} catch (error) {{
                console.error('Download failed:', error);
                showStatus('Download failed: ' + error.message, 'error');
                resetButton();
            }}
        }}

        function showStatus(message, type) {{
            statusMessage.textContent = message;
            statusMessage.className = 'status-message ' + type;
            statusMessage.style.display = 'block';
        }}

        function resetButton() {{
            downloadBtn.disabled = false;
            downloadBtn.textContent = '⬇️ Download Now';
        }}

        // Listen for download completion from parent
        window.addEventListener('message', function(event) {{
            // Security: Only accept messages from the parent window
            if (event.source !== window.parent) {{
                return;
            }}

            if (event.data.type === 'download_complete') {{
                if (event.data.success) {{
                    showStatus('✅ Download completed successfully!', 'success');
                }} else {{
                    showStatus('❌ Download failed: ' + event.data.error, 'error');
                }}
                resetButton();
            }}
        }});
    </script>
</body>
</html>"#,
        html_escape::encode_text(title),
        html_escape::encode_text(title),
        html_escape::encode_text(export_type),
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        files.len(),
        files_list,
        html_escape::encode_text(download_path)
    )
}
