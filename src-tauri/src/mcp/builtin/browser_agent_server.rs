use crate::mcp::{
    builtin::BuiltinMCPServer, JSONSchema, JSONSchemaType, MCPError, MCPResponse, MCPTool,
};
use crate::services::InteractiveBrowserServer;
use chrono;
use log::{info, warn};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::AppHandle;
use uuid::Uuid;

/// Comprehensive browser automation server providing web crawling,
/// interactive browser sessions, and advanced web automation capabilities
pub struct BrowserAgentServer {
    browser_server: Arc<InteractiveBrowserServer>,
}

impl BrowserAgentServer {
    pub fn new(app_handle: AppHandle) -> Self {
        let browser_server = Arc::new(InteractiveBrowserServer::new(app_handle));
        Self { browser_server }
    }

    /// Get the base directory compatible with SecurityValidator
    fn get_base_dir() -> PathBuf {
        if let Ok(root) = std::env::var("SYNAPTICFLOW_PROJECT_ROOT") {
            PathBuf::from(root)
        } else {
            std::env::temp_dir().join("synaptic-flow")
        }
    }

    /// Convert absolute path to relative path for MCP filesystem compatibility
    fn make_relative_path(&self, absolute_path: &Path) -> Result<String, String> {
        let base_dir = Self::get_base_dir();

        match absolute_path.strip_prefix(&base_dir) {
            Ok(relative) => {
                let rel_str = relative.to_string_lossy().to_string();
                if rel_str.is_empty() {
                    // Fallback: crawl_cache/filename 형태로 반환
                    if let Some(file_name) = absolute_path.file_name() {
                        Ok(format!("crawl_cache/{}", file_name.to_string_lossy()))
                    } else {
                        Err("Failed to derive relative path".to_string())
                    }
                } else {
                    Ok(rel_str)
                }
            }
            Err(_) => {
                // base_dir 외부 파일인 경우 안전상 경로 노출 금지
                warn!(
                    "File is outside allowed base directory: {:?}",
                    absolute_path
                );
                if let Some(file_name) = absolute_path.file_name() {
                    Ok(format!("crawl_cache/{}", file_name.to_string_lossy()))
                } else {
                    Err("File is outside allowed base directory".to_string())
                }
            }
        }
    }

    /// Get app temp directory for saving crawl results (compatible with SecurityValidator base_dir)
    async fn get_crawl_temp_dir(&self) -> Result<PathBuf, String> {
        let base_dir = Self::get_base_dir();
        let crawl_dir = base_dir.join("crawl_cache");

        if !crawl_dir.exists() {
            tokio::fs::create_dir_all(&crawl_dir)
                .await
                .map_err(|e| format!("Failed to create crawl directory: {}", e))?;
        }

        Ok(crawl_dir)
    }

    /// Generate unique hash for the crawled content
    fn generate_content_hash(url: &str, content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        hasher.update(content.as_bytes());
        hasher.update(chrono::Utc::now().to_rfc3339().as_bytes());
        format!("{:x}", hasher.finalize())[..16].to_string()
    }

    /// Save crawled HTML content to temp file
    async fn save_crawl_result(
        &self,
        url: &str,
        html_content: &str,
        extracted_data: &serde_json::Value,
    ) -> Result<PathBuf, String> {
        let crawl_dir = self.get_crawl_temp_dir().await?;
        let content_hash = Self::generate_content_hash(url, html_content);
        let file_name = format!("{}.html", content_hash);
        let file_path = crawl_dir.join(file_name);

        let enhanced_html = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>SynapticFlow Crawl Result</title>
    <style>
        .synaptic-metadata {{
            background: #f5f5f5;
            border: 1px solid #ddd;
            padding: 15px;
            margin: 20px 0;
            border-radius: 5px;
            font-family: monospace;
        }}
        .synaptic-original {{
            border-top: 2px solid #007acc;
            margin-top: 20px;
        }}
    </style>
</head>
<body>
    <div class="synaptic-metadata">
        <h2>🕷️ SynapticFlow Crawl Metadata</h2>
        <p><strong>URL:</strong> {}</p>
        <p><strong>Crawled at:</strong> {}</p>
        <p><strong>Content Hash:</strong> {}</p>
        <details>
            <summary><strong>Extracted Data</strong></summary>
            <pre>{}</pre>
        </details>
    </div>

    <div class="synaptic-original">
        <h2>📄 Original Content</h2>
        {}
    </div>
</body>
</html>"#,
            html_escape::encode_text(url),
            chrono::Utc::now().to_rfc3339(),
            content_hash,
            serde_json::to_string_pretty(extracted_data).unwrap_or_default(),
            html_content
        );

        tokio::fs::write(&file_path, enhanced_html)
            .await
            .map_err(|e| format!("Failed to save crawl result: {}", e))?;

        info!("Saved crawl result to: {:?}", file_path);
        Ok(file_path)
    }

    /// Handle screenshot tool call
    async fn handle_screenshot(&self, _args: Value) -> MCPResponse {
        let request_id = Value::String(Uuid::new_v4().to_string());

        MCPResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(request_id),
            result: None,
            error: Some(MCPError {
                code: -32601,
                message: "Screenshot functionality not yet implemented".to_string(),
                data: None,
            }),
        }
    }

    /// Save extracted data to JSON file
    async fn save_extracted_data(
        &self,
        session_id: &str,
        script: &str,
        extracted_data: &Value,
    ) -> Result<PathBuf, String> {
        let crawl_dir = self.get_crawl_temp_dir().await?;

        // Generate unique filename based on session and timestamp
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let session_short = &session_id[..8.min(session_id.len())];
        let file_name = format!("extraction_{}_{}.json", session_short, timestamp);
        let file_path = crawl_dir.join(file_name);

        // Create extraction result structure
        let extraction_result = json!({
            "session_id": session_id,
            "script": script,
            "extracted_data": extracted_data,
            "extraction_timestamp": chrono::Utc::now().to_rfc3339(),
            "synaptic_flow_version": "1.0.0"
        });

        // Save as formatted JSON
        let json_content = serde_json::to_string_pretty(&extraction_result)
            .map_err(|e| format!("Failed to serialize extraction data: {}", e))?;

        tokio::fs::write(&file_path, json_content)
            .await
            .map_err(|e| format!("Failed to save extraction data: {}", e))?;

        info!("Saved extraction data to: {:?}", file_path);
        Ok(file_path)
    }

    /// Handle extract_data tool call
    async fn handle_extract_data(&self, args: Value) -> MCPResponse {
        let request_id = Value::String(Uuid::new_v4().to_string());

        let session_id = match args.get("session_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32602,
                        message: "Missing required parameter: session_id".to_string(),
                        data: None,
                    }),
                };
            }
        };

        let script = match args.get("script").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32602,
                        message: "Missing required parameter: script".to_string(),
                        data: None,
                    }),
                };
            }
        };

        match self.browser_server.execute_script(session_id, script).await {
            Ok(result) => {
                // Try to parse result as JSON, fallback to string
                let parsed_result = match serde_json::from_str::<Value>(&result) {
                    Ok(json_val) => json_val,
                    Err(_) => json!(result), // Parsing failed, treat as string
                };

                // Save extracted data to file
                match self
                    .save_extracted_data(session_id, script, &parsed_result)
                    .await
                {
                    Ok(file_path) => {
                        // ✅ SecurityValidator 호환 상대경로 생성
                        match self.make_relative_path(&file_path) {
                            Ok(relative_path) => {
                                MCPResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: Some(request_id),
                                    result: Some(json!({
                                        "content": [{
                                            "type": "text",
                                            "text": format!(
                                                "✅ Data extraction successful from session: {}\n💾 Extracted data saved to: {}\n📊 Data preview: {}",
                                                session_id,
                                                relative_path, // ✅ 상대경로만 노출
                                                serde_json::to_string_pretty(&parsed_result)
                                                    .unwrap_or_else(|_| "Unable to preview data".to_string())
                                                    .chars().take(200).collect::<String>() + "..."
                                            )
                                        }]
                                    })),
                                    error: None,
                                }
                            }
                            Err(e) => {
                                warn!("Failed to create relative path: {}", e);
                                // ✅ 상대경로 생성 실패 시 경로 노출하지 않음
                                MCPResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: Some(request_id),
                                    result: Some(json!({
                                        "content": [{
                                            "type": "text",
                                            "text": format!(
                                                "✅ Data extraction successful from session: {}\n� Data saved to file (path unavailable)\n📊 Data preview: {}",
                                                session_id,
                                                serde_json::to_string_pretty(&parsed_result)
                                                    .unwrap_or_else(|_| "Unable to preview data".to_string())
                                                    .chars().take(200).collect::<String>() + "..."
                                            )
                                        }]
                                    })),
                                    error: None,
                                }
                            }
                        }
                    }
                    Err(save_error) => {
                        // File save failed, but still return the data in content
                        warn!("Failed to save extraction data: {}", save_error);
                        MCPResponse {
                            jsonrpc: "2.0".to_string(),
                            id: Some(request_id),
                            result: Some(json!({
                                "content": [{
                                    "type": "text",
                                    "text": format!(
                                        "✅ Data extraction successful from session: {}\n⚠️ File save failed: {}\n📊 Data: {}",
                                        session_id,
                                        save_error,
                                        serde_json::to_string_pretty(&parsed_result)
                                            .unwrap_or_else(|_| "Unable to display data".to_string())
                                            .chars().take(200).collect::<String>() + "..."
                                    )
                                }]
                            })),
                            error: None,
                        }
                    }
                }
            }
            Err(e) => MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: None,
                error: Some(MCPError {
                    code: -32603,
                    message: format!("Script execution failed: {}", e),
                    data: None,
                }),
            },
        }
    }

    /// Handle create_browser_session tool call
    async fn handle_create_browser_session(&self, args: Value) -> MCPResponse {
        let request_id = Value::String(Uuid::new_v4().to_string());

        let url = match args.get("url").and_then(|v| v.as_str()) {
            Some(url) => url,
            None => {
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32602,
                        message: "Missing required parameter: url".to_string(),
                        data: None,
                    }),
                };
            }
        };

        let title = args.get("title").and_then(|v| v.as_str());

        match self.browser_server.create_browser_session(url, title).await {
            Ok(session_id) => MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: Some(json!({
                    "content": [{
                        "type": "text",
                        "text": format!("✅ Browser session created successfully: {}\n🌐 URL: {}\n📝 Title: {}", session_id, url, title.unwrap_or("Browser Session"))
                    }]
                })),
                error: None,
            },
            Err(e) => MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: None,
                error: Some(MCPError {
                    code: -32603,
                    message: format!("Failed to create browser session: {}", e),
                    data: None,
                }),
            },
        }
    }

    /// Handle click_element tool call
    async fn handle_click_element(&self, args: Value) -> MCPResponse {
        let request_id = Value::String(Uuid::new_v4().to_string());

        let session_id = match args.get("session_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32602,
                        message: "Missing required parameter: session_id".to_string(),
                        data: None,
                    }),
                };
            }
        };

        let selector = match args.get("selector").and_then(|v| v.as_str()) {
            Some(sel) => sel,
            None => {
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32602,
                        message: "Missing required parameter: selector".to_string(),
                        data: None,
                    }),
                };
            }
        };

        match self
            .browser_server
            .click_element(session_id, selector)
            .await
        {
            Ok(result) => MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: Some(json!({
                    "content": [{
                        "type": "text",
                        "text": format!("✅ Element clicked successfully: {}\n🎯 Selector: {}", result, selector)
                    }]
                })),
                error: None,
            },
            Err(e) => MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: None,
                error: Some(MCPError {
                    code: -32603,
                    message: format!("Failed to click element: {}", e),
                    data: None,
                }),
            },
        }
    }

    /// Handle input_text tool call
    async fn handle_input_text(&self, args: Value) -> MCPResponse {
        let request_id = Value::String(Uuid::new_v4().to_string());

        let session_id = match args.get("session_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32602,
                        message: "Missing required parameter: session_id".to_string(),
                        data: None,
                    }),
                };
            }
        };

        let selector = match args.get("selector").and_then(|v| v.as_str()) {
            Some(sel) => sel,
            None => {
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32602,
                        message: "Missing required parameter: selector".to_string(),
                        data: None,
                    }),
                };
            }
        };

        let text = match args.get("text").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => {
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32602,
                        message: "Missing required parameter: text".to_string(),
                        data: None,
                    }),
                };
            }
        };

        match self
            .browser_server
            .input_text(session_id, selector, text)
            .await
        {
            Ok(result) => MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: Some(json!({
                    "content": [{
                        "type": "text",
                        "text": format!("✅ Text input successful: {}\n📝 Text: '{}' into selector: {}", result, text, selector)
                    }]
                })),
                error: None,
            },
            Err(e) => MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: None,
                error: Some(MCPError {
                    code: -32603,
                    message: format!("Failed to input text: {}", e),
                    data: None,
                }),
            },
        }
    }

    /// Handle navigate_url tool call
    async fn handle_navigate_url(&self, args: Value) -> MCPResponse {
        let request_id = Value::String(Uuid::new_v4().to_string());

        let session_id = match args.get("session_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32602,
                        message: "Missing required parameter: session_id".to_string(),
                        data: None,
                    }),
                };
            }
        };

        let url = match args.get("url").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => {
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32602,
                        message: "Missing required parameter: url".to_string(),
                        data: None,
                    }),
                };
            }
        };

        match self.browser_server.navigate_to_url(session_id, url).await {
            Ok(result) => MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: Some(json!({
                    "content": [{
                        "type": "text",
                        "text": format!("✅ Navigation successful: {}\n🌐 Navigated to: {}", result, url)
                    }]
                })),
                error: None,
            },
            Err(e) => MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: None,
                error: Some(MCPError {
                    code: -32603,
                    message: format!("Failed to navigate: {}", e),
                    data: None,
                }),
            },
        }
    }

    /// Handle crawl_current_page tool call
    async fn handle_crawl_current_page(&self, args: Value) -> MCPResponse {
        let request_id = Value::String(Uuid::new_v4().to_string());

        let session_id = match args.get("session_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32602,
                        message: "Missing required parameter: session_id".to_string(),
                        data: None,
                    }),
                };
            }
        };

        let selectors = args
            .get("selectors")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_else(|| vec!["body".to_string()]);

        let save_html = args
            .get("save_html")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let wait_for_content = args
            .get("wait_for_content")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        info!(
            "Crawling current page for session {} with {} selectors",
            session_id,
            selectors.len()
        );

        // Wait for content to load if requested
        if wait_for_content {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }

        // Get current URL for context
        let current_url = self
            .browser_server
            .get_current_url(session_id)
            .await
            .unwrap_or_else(|_| "Unknown URL".to_string());

        // Extract data using CSS selectors
        let mut extracted_data = serde_json::Map::new();

        for selector in &selectors {
            let script = format!(
                r#"
                (function() {{
                    try {{
                        const elements = document.querySelectorAll('{}');
                        const results = Array.from(elements).map(el => ({{
                            tagName: el.tagName,
                            textContent: el.textContent?.trim() || '',
                            innerHTML: el.innerHTML.length > 500 ? el.innerHTML.substring(0, 500) + '...' : el.innerHTML,
                            attributes: Array.from(el.attributes).reduce((acc, attr) => {{
                                acc[attr.name] = attr.value;
                                return acc;
                            }}, {{}})
                        }}));
                        return results;
                    }} catch (error) {{
                        return [{{ error: error.message }}];
                    }}
                }})()
                "#,
                selector.replace('"', r#"\""#)
            );

            match self
                .browser_server
                .execute_script(session_id, &script)
                .await
            {
                Ok(result) => {
                    // Try to parse the JSON result from the refactored execute_script
                    if let Ok(parsed_result) = serde_json::from_str::<serde_json::Value>(&result) {
                        extracted_data.insert(selector.clone(), parsed_result);
                    } else {
                        // If not JSON, treat as plain text result
                        extracted_data.insert(selector.clone(), json!(result));
                    }
                }
                Err(e) => {
                    warn!("Failed to extract data for selector '{}': {}", selector, e);
                    extracted_data.insert(selector.clone(), json!({"error": e}));
                }
            }
        }

        // Get the full page HTML for saving if requested
        let html_content = if save_html {
            match self.browser_server.get_page_content(session_id).await {
                Ok(content) => Some(content),
                Err(e) => {
                    warn!("Failed to get page content: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Create response message
        let mut status_message = format!(
            "✅ Successfully crawled current page from session: {}\n🌐 URL: {}\n📄 Extracted {} selectors\n📊 Extraction results:",
            session_id,
            current_url,
            selectors.len()
        );

        // Add extracted data preview to content
        for selector in selectors.iter() {
            if let Some(data) = extracted_data.get(selector) {
                let preview = if data.is_array() {
                    let array = data.as_array().unwrap();
                    format!("{} elements found", array.len())
                } else if data.is_object() {
                    if data.get("error").is_some() {
                        "Error during extraction".to_string()
                    } else {
                        "Object data".to_string()
                    }
                } else {
                    let text = data.as_str().unwrap_or("Unknown");
                    if text.len() > 100 {
                        format!("\"{}...\"", &text[..97])
                    } else {
                        format!("\"{}\"", text)
                    }
                };
                status_message.push_str(&format!("\n  {}: {}", selector, preview));
            }
        }

        // Save HTML if requested and available
        if let Some(html) = html_content {
            match self
                .save_crawl_result(&current_url, &html, &json!(extracted_data))
                .await
            {
                Ok(saved_path) => match self.make_relative_path(&saved_path) {
                    Ok(relative_path) => {
                        status_message.push_str(&format!("\n💾 Saved HTML to: {}", relative_path));
                    }
                    Err(e) => {
                        warn!("Failed to convert to relative path: {}", e);
                        status_message.push_str("\n💾 HTML saved to file");
                    }
                },
                Err(e) => {
                    warn!("Failed to save crawl result: {}", e);
                    status_message.push_str(&format!("\n⚠️ Warning: Failed to save HTML: {}", e));
                }
            }
        }

        MCPResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(request_id),
            result: Some(json!({
                "content": [{
                    "type": "text",
                    "text": status_message
                }]
            })),
            error: None,
        }
    }

    /// Create screenshot tool definition
    fn create_screenshot_tool(&self) -> MCPTool {
        let mut properties = HashMap::new();

        properties.insert(
            "url".to_string(),
            JSONSchema {
                schema_type: JSONSchemaType::String {
                    min_length: None,
                    max_length: None,
                    pattern: None,
                    format: None,
                },
                title: None,
                description: Some("The URL to capture".to_string()),
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
        );

        MCPTool {
            name: "screenshot".to_string(),
            title: Some("WebView Screenshot".to_string()),
            description: "Take a screenshot of a web page".to_string(),
            input_schema: JSONSchema {
                schema_type: JSONSchemaType::Object {
                    properties: Some(properties),
                    required: Some(vec!["url".to_string()]),
                    additional_properties: None,
                    min_properties: None,
                    max_properties: None,
                },
                title: None,
                description: None,
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
            output_schema: None,
            annotations: None,
        }
    }

    /// Create extract_data tool definition
    fn create_extract_data_tool(&self) -> MCPTool {
        let mut properties = HashMap::new();

        properties.insert(
            "session_id".to_string(),
            JSONSchema {
                schema_type: JSONSchemaType::String {
                    min_length: None,
                    max_length: None,
                    pattern: None,
                    format: None,
                },
                title: None,
                description: Some("Browser session ID".to_string()),
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
        );

        properties.insert(
            "script".to_string(),
            JSONSchema {
                schema_type: JSONSchemaType::String {
                    min_length: None,
                    max_length: None,
                    pattern: None,
                    format: None,
                },
                title: None,
                description: Some("JavaScript code to execute for data extraction".to_string()),
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
        );

        MCPTool {
            name: "extract_data".to_string(),
            title: Some("WebView Data Extractor".to_string()),
            description: "Execute JavaScript code in a browser session and extract data"
                .to_string(),
            input_schema: JSONSchema {
                schema_type: JSONSchemaType::Object {
                    properties: Some(properties),
                    required: Some(vec!["session_id".to_string(), "script".to_string()]),
                    additional_properties: None,
                    min_properties: None,
                    max_properties: None,
                },
                title: None,
                description: None,
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
            output_schema: None,
            annotations: None,
        }
    }

    /// Create browser session tool definition
    fn create_browser_session_tool(&self) -> MCPTool {
        let mut properties = HashMap::new();

        properties.insert(
            "url".to_string(),
            JSONSchema {
                schema_type: JSONSchemaType::String {
                    min_length: None,
                    max_length: None,
                    pattern: None,
                    format: None,
                },
                title: None,
                description: Some("The URL to navigate to".to_string()),
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
        );

        properties.insert(
            "title".to_string(),
            JSONSchema {
                schema_type: JSONSchemaType::String {
                    min_length: None,
                    max_length: None,
                    pattern: None,
                    format: None,
                },
                title: None,
                description: Some("Optional title for the browser session".to_string()),
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
        );

        MCPTool {
            name: "create_browser_session".to_string(),
            title: Some("Create Browser Session".to_string()),
            description: "Create a new interactive browser session in a separate window"
                .to_string(),
            input_schema: JSONSchema {
                schema_type: JSONSchemaType::Object {
                    properties: Some(properties),
                    required: Some(vec!["url".to_string()]),
                    additional_properties: None,
                    min_properties: None,
                    max_properties: None,
                },
                title: None,
                description: None,
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
            output_schema: None,
            annotations: None,
        }
    }

    /// Create click element tool definition
    fn create_click_element_tool(&self) -> MCPTool {
        let mut properties = HashMap::new();

        properties.insert(
            "session_id".to_string(),
            JSONSchema {
                schema_type: JSONSchemaType::String {
                    min_length: None,
                    max_length: None,
                    pattern: None,
                    format: None,
                },
                title: None,
                description: Some("Browser session ID".to_string()),
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
        );

        properties.insert(
            "selector".to_string(),
            JSONSchema {
                schema_type: JSONSchemaType::String {
                    min_length: None,
                    max_length: None,
                    pattern: None,
                    format: None,
                },
                title: None,
                description: Some("CSS selector for the element to click".to_string()),
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
        );

        MCPTool {
            name: "click_element".to_string(),
            title: Some("Click Element".to_string()),
            description: "Click on a DOM element in the browser session".to_string(),
            input_schema: JSONSchema {
                schema_type: JSONSchemaType::Object {
                    properties: Some(properties),
                    required: Some(vec!["session_id".to_string(), "selector".to_string()]),
                    additional_properties: None,
                    min_properties: None,
                    max_properties: None,
                },
                title: None,
                description: None,
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
            output_schema: None,
            annotations: None,
        }
    }

    /// Create input text tool definition
    fn create_input_text_tool(&self) -> MCPTool {
        let mut properties = HashMap::new();

        properties.insert(
            "session_id".to_string(),
            JSONSchema {
                schema_type: JSONSchemaType::String {
                    min_length: None,
                    max_length: None,
                    pattern: None,
                    format: None,
                },
                title: None,
                description: Some("Browser session ID".to_string()),
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
        );

        properties.insert(
            "selector".to_string(),
            JSONSchema {
                schema_type: JSONSchemaType::String {
                    min_length: None,
                    max_length: None,
                    pattern: None,
                    format: None,
                },
                title: None,
                description: Some("CSS selector for the input element".to_string()),
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
        );

        properties.insert(
            "text".to_string(),
            JSONSchema {
                schema_type: JSONSchemaType::String {
                    min_length: None,
                    max_length: None,
                    pattern: None,
                    format: None,
                },
                title: None,
                description: Some("Text to input".to_string()),
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
        );

        MCPTool {
            name: "input_text".to_string(),
            title: Some("Input Text".to_string()),
            description: "Input text into a form field".to_string(),
            input_schema: JSONSchema {
                schema_type: JSONSchemaType::Object {
                    properties: Some(properties),
                    required: Some(vec![
                        "session_id".to_string(),
                        "selector".to_string(),
                        "text".to_string(),
                    ]),
                    additional_properties: None,
                    min_properties: None,
                    max_properties: None,
                },
                title: None,
                description: None,
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
            output_schema: None,
            annotations: None,
        }
    }

    /// Create navigate URL tool definition
    fn create_navigate_url_tool(&self) -> MCPTool {
        let mut properties = HashMap::new();

        properties.insert(
            "session_id".to_string(),
            JSONSchema {
                schema_type: JSONSchemaType::String {
                    min_length: None,
                    max_length: None,
                    pattern: None,
                    format: None,
                },
                title: None,
                description: Some("Browser session ID".to_string()),
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
        );

        properties.insert(
            "url".to_string(),
            JSONSchema {
                schema_type: JSONSchemaType::String {
                    min_length: None,
                    max_length: None,
                    pattern: None,
                    format: None,
                },
                title: None,
                description: Some("URL to navigate to".to_string()),
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
        );

        MCPTool {
            name: "navigate_url".to_string(),
            title: Some("Navigate to URL".to_string()),
            description: "Navigate to a new URL in an existing browser session".to_string(),
            input_schema: JSONSchema {
                schema_type: JSONSchemaType::Object {
                    properties: Some(properties),
                    required: Some(vec!["session_id".to_string(), "url".to_string()]),
                    additional_properties: None,
                    min_properties: None,
                    max_properties: None,
                },
                title: None,
                description: None,
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
            output_schema: None,
            annotations: None,
        }
    }

    /// Create crawl_current_page tool definition
    fn create_crawl_current_page_tool(&self) -> MCPTool {
        let mut properties = HashMap::new();

        properties.insert(
            "session_id".to_string(),
            JSONSchema {
                schema_type: JSONSchemaType::String {
                    min_length: None,
                    max_length: None,
                    pattern: None,
                    format: None,
                },
                title: None,
                description: Some("Browser session ID to crawl from".to_string()),
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
        );

        properties.insert(
            "selectors".to_string(),
            JSONSchema {
                schema_type: JSONSchemaType::Array {
                    items: Some(Box::new(JSONSchema {
                        schema_type: JSONSchemaType::String {
                            min_length: None,
                            max_length: None,
                            pattern: None,
                            format: None,
                        },
                        title: None,
                        description: None,
                        default: None,
                        examples: None,
                        enum_values: None,
                        const_value: None,
                    })),
                    min_items: None,
                    max_items: None,
                    unique_items: None,
                },
                title: None,
                description: Some(
                    "CSS selectors to extract data from the current page".to_string(),
                ),
                default: Some(json!(["body"])),
                examples: Some(vec![json!(["h1", ".article-content", "#main-content"])]),
                enum_values: None,
                const_value: None,
            },
        );

        properties.insert(
            "save_html".to_string(),
            JSONSchema {
                schema_type: JSONSchemaType::Boolean,
                title: None,
                description: Some("Whether to save the full HTML content to file".to_string()),
                default: Some(json!(true)),
                examples: None,
                enum_values: None,
                const_value: None,
            },
        );

        properties.insert(
            "wait_for_content".to_string(),
            JSONSchema {
                schema_type: JSONSchemaType::Boolean,
                title: None,
                description: Some("Whether to wait for dynamic content to load".to_string()),
                default: Some(json!(false)),
                examples: None,
                enum_values: None,
                const_value: None,
            },
        );

        MCPTool {
            name: "crawl_current_page".to_string(),
            title: Some("Crawl Current Page".to_string()),
            description: "Extract data from the current page of an existing browser session using CSS selectors".to_string(),
            input_schema: JSONSchema {
                schema_type: JSONSchemaType::Object {
                    properties: Some(properties),
                    required: Some(vec!["session_id".to_string()]),
                    additional_properties: None,
                    min_properties: None,
                    max_properties: None,
                },
                title: None,
                description: None,
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
            output_schema: None,
            annotations: None,
        }
    }
}

#[async_trait::async_trait]
impl BuiltinMCPServer for BrowserAgentServer {
    fn name(&self) -> &str {
        "builtin.browser_agent"
    }

    fn description(&self) -> &str {
        "Built-in browser automation and web crawling agent server"
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![
            self.create_crawl_current_page_tool(),
            self.create_screenshot_tool(),
            self.create_extract_data_tool(),
            self.create_browser_session_tool(),
            self.create_click_element_tool(),
            self.create_input_text_tool(),
            self.create_navigate_url_tool(),
        ]
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse {
        match tool_name {
            "crawl_current_page" => self.handle_crawl_current_page(args).await,
            "screenshot" => self.handle_screenshot(args).await,
            "extract_data" => self.handle_extract_data(args).await,
            "create_browser_session" => self.handle_create_browser_session(args).await,
            "click_element" => self.handle_click_element(args).await,
            "input_text" => self.handle_input_text(args).await,
            "navigate_url" => self.handle_navigate_url(args).await,
            _ => {
                let request_id = Value::String(Uuid::new_v4().to_string());
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32601,
                        message: format!("Tool '{}' not found in browser agent server", tool_name),
                        data: None,
                    }),
                }
            }
        }
    }
}
