use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, ToolGroup,
};
use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{MCPContent, MCPResult, ServiceContext};
use crate::mcp::MCPTool;
use async_trait::async_trait;
use handlebars::Handlebars;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

pub mod tools;

// Embed templates
const BAR_CHART_TEMPLATE: &str = include_str!("templates/bar-chart.hbs");
const CIRCUIT_BREAK_TEMPLATE: &str = include_str!("templates/circuit-break.hbs");
const LINE_CHART_TEMPLATE: &str = include_str!("templates/line-chart.hbs");
const PRESENT_INTERACTIVE_TEMPLATE: &str = include_str!("templates/present-interactive.hbs");

#[derive(Debug)]
pub struct UiServer {
    handlebars: Arc<Mutex<Handlebars<'static>>>,
}

fn summarize_interactive_content(content: &str, max_chars: usize) -> Option<String> {
    let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");

    if normalized.is_empty() {
        return None;
    }

    let excerpt: String = normalized.chars().take(max_chars).collect();

    if normalized.chars().count() > max_chars {
        Some(format!("{}...", excerpt))
    } else {
        Some(excerpt)
    }
}

impl Default for UiServer {
    fn default() -> Self {
        Self::new()
    }
}

impl UiServer {
    pub fn new() -> Self {
        let mut handlebars = Handlebars::new();

        // Register templates
        handlebars
            .register_template_string("bar-chart", BAR_CHART_TEMPLATE)
            .unwrap();
        handlebars
            .register_template_string("circuit-break", CIRCUIT_BREAK_TEMPLATE)
            .unwrap();
        handlebars
            .register_template_string("line-chart", LINE_CHART_TEMPLATE)
            .unwrap();
        handlebars
            .register_template_string("present-interactive", PRESENT_INTERACTIVE_TEMPLATE)
            .unwrap();

        Self {
            handlebars: Arc::new(Mutex::new(handlebars)),
        }
    }

    fn get_user_answer(&self, args: Value) -> Result<MCPResult, String> {
        let _message_id = match args.get("messageId").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => return Ok(missing_param_error("messageId", ToolGroup::UI)),
        };

        let answer = args.get("answer");
        let cancelled = args
            .get("cancelled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if cancelled {
            return Ok(MCPResult::informational("User cancelled the prompt."));
        }

        let answer_str = if let Some(ans) = answer {
            if ans.is_null() {
                "null".to_string()
            } else {
                serde_json::to_string(ans).unwrap_or_else(|_| "invalid".to_string())
            }
        } else {
            "null".to_string()
        };

        Ok(MCPResult {
            content: Some(vec![MCPContent::Text {
                text: format!("User replied: {}", answer_str),
                is_error: None,
            }]),
            structured_content: None,
            is_error: Some(false),
        })
    }

    fn visualize_data(&self, args: Value) -> Result<MCPResult, String> {
        let type_ = match args.get("type").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => return Ok(missing_param_error("type", ToolGroup::UI)),
        };

        // Validate type
        if !["bar", "line"].contains(&type_) {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Invalid visualization type '{}'. Supported: bar, line",
                    type_
                ),
                ToolGroup::UI,
            )
            .with_guidance(vec![
                "Choose 'bar' for categories or 'line' for trends".to_string()
            ])
            .to_mcp_result());
        }

        let data_points = match args.get("data").and_then(|v| v.as_array()) {
            Some(arr) => arr,
            None => return Ok(missing_param_error("data", ToolGroup::UI)),
        };

        if data_points.is_empty() {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Visualization data array cannot be empty",
                ToolGroup::UI,
            )
            .with_guidance(vec![
                "Provide at least one data point with label and value".to_string()
            ])
            .to_mcp_result());
        }

        let width = 600;
        let height = 300;
        let padding = 40;

        // Parse data
        struct DataPoint {
            label: String,
            value: f64,
        }

        let mut parsed_data = Vec::new();
        let mut max_value = f64::MIN;
        let mut min_value = 0.0; // Start from 0 for bar charts usually

        for point in data_points {
            let label = point
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let value = point.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
            if value > max_value {
                max_value = value;
            }
            if value < min_value {
                min_value = value;
            }
            parsed_data.push(DataPoint { label, value });
        }

        if max_value == min_value {
            max_value = min_value + 1.0;
        }

        let range = max_value - min_value;
        let available_height = (height - 2 * padding) as f64;
        let available_width = (width - 2 * padding) as f64;

        let mut template_data = json!({
            "svgWidth": width,
            "svgHeight": height,
        });

        match type_ {
            "bar" => {
                let bar_width = available_width / parsed_data.len() as f64;
                let bar_gap = bar_width * 0.2;
                let actual_bar_width = bar_width - bar_gap;

                let mut bars_html = String::new();

                for (i, point) in parsed_data.iter().enumerate() {
                    let x = padding as f64 + (i as f64 * bar_width) + (bar_gap / 2.0);
                    let bar_height = ((point.value - min_value) / range) * available_height;
                    let y = (height - padding) as f64 - bar_height;

                    // Bar rect
                    bars_html.push_str(&format!(
                        r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" rx="4" />"#,
                        x, y, actual_bar_width, bar_height, "#3b82f6"
                    ));

                    // Label
                    bars_html.push_str(&format!(
                        "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"12\" fill=\"#6b7280\">{}</text>",
                        x + actual_bar_width / 2.0, height - padding + 20, html_escape::encode_text(&point.label)
                    ));

                    // Value
                    bars_html.push_str(&format!(
                        "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"12\" fill=\"#374151\">{}</text>",
                        x + actual_bar_width / 2.0, y - 5.0, point.value
                    ));
                }

                template_data
                    .as_object_mut()
                    .unwrap()
                    .insert("barsHtml".to_string(), json!(bars_html));

                let handlebars = self.handlebars.lock().unwrap();
                let html = match handlebars.render("bar-chart", &template_data) {
                    Ok(h) => h,
                    Err(e) => {
                        return Ok(guided_error(
                            ErrorCategory::OperationFailed,
                            format!("Failed to render bar chart: {}", e),
                            ToolGroup::UI,
                        )
                        .with_guidance(vec![
                            "Verify data format (label/value pairs) is correct".to_string(),
                            "Ensure all values are numeric".to_string(),
                        ])
                        .to_mcp_result());
                    }
                };

                Ok(crate::mcp::builtin::utils::create_resource_response(
                    &format!("ui://chart/{}", uuid::Uuid::new_v4()),
                    "text/html",
                    &html,
                    "ui",
                    "visualizeData",
                    None,
                ))
            }
            "line" => {
                let step_x = available_width / (parsed_data.len() - 1).max(1) as f64;

                let mut points_str = String::new();
                let mut labels_html = String::new();

                for (i, point) in parsed_data.iter().enumerate() {
                    let x = padding as f64 + (i as f64 * step_x);
                    let y = (height - padding) as f64
                        - (((point.value - min_value) / range) * available_height);

                    if i > 0 {
                        points_str.push(' ');
                    }
                    points_str.push_str(&format!("{},{}", x, y));

                    // Point circle
                    labels_html.push_str(&format!(
                        "<circle cx=\"{}\" cy=\"{}\" r=\"4\" fill=\"#3b82f6\" />",
                        x, y
                    ));

                    // Label
                    labels_html.push_str(&format!(
                        "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"12\" fill=\"#6b7280\">{}</text>",
                        x, height - padding + 20, html_escape::encode_text(&point.label)
                    ));

                    // Value
                    labels_html.push_str(&format!(
                        "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"12\" fill=\"#374151\">{}</text>",
                        x, y - 10.0, point.value
                    ));
                }

                template_data
                    .as_object_mut()
                    .unwrap()
                    .insert("points".to_string(), json!(points_str));
                template_data
                    .as_object_mut()
                    .unwrap()
                    .insert("labelsHtml".to_string(), json!(labels_html));

                let handlebars = self.handlebars.lock().unwrap();
                let html = match handlebars.render("line-chart", &template_data) {
                    Ok(h) => h,
                    Err(e) => {
                        return Ok(guided_error(
                            ErrorCategory::OperationFailed,
                            format!("Failed to render line chart: {}", e),
                            ToolGroup::UI,
                        )
                        .with_guidance(vec![
                            "Verify data format (label/value pairs) is correct".to_string(),
                            "Ensure all values are numeric".to_string(),
                        ])
                        .to_mcp_result());
                    }
                };

                Ok(crate::mcp::builtin::utils::create_resource_response(
                    &format!("ui://chart/{}", uuid::Uuid::new_v4()),
                    "text/html",
                    &html,
                    "ui",
                    "visualizeData",
                    None,
                ))
            }
            _ => Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!("Unsupported visualization type: {}", type_),
                ToolGroup::UI,
            )
            .with_guidance(vec!["Supported types: bar, line".to_string()])
            .to_mcp_result()),
        }
    }
    fn circuit_break(&self, args: Value) -> Result<MCPResult, String> {
        let tool_name = match args.get("toolName").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => return Ok(missing_param_error("toolName", ToolGroup::UI)),
        };
        let repetition_count = args
            .get("repetitionCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let args_str = args.get("args").and_then(|v| v.as_str()).unwrap_or("");

        let context_json = json!({
            "toolName": tool_name,
            "repetitionCount": repetition_count,
            "args": args_str,
        });

        let data = json!({
            "toolName": tool_name,
            "repetitionCount": repetition_count,
            "args": args_str,
            "contextJson": context_json.to_string(),
        });

        let handlebars = self.handlebars.lock().unwrap();
        let html = match handlebars.render("circuit-break", &data) {
            Ok(h) => h,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::OperationFailed,
                    format!("Failed to render circuit breaker: {}", e),
                    ToolGroup::UI,
                )
                .with_guidance(vec![
                    "Verify internal tool name and arguments are valid".to_string()
                ])
                .to_mcp_result());
            }
        };

        let warning_message = format!(
            "⚠️ Circuit Breaker Triggered\n\nThe tool \"{}\" has been called {} times consecutively with identical parameters.\n\nThis usually indicates the agent is stuck in a loop. Please review the situation and click Resume to continue.",
            tool_name, repetition_count
        );

        Ok(crate::mcp::builtin::utils::create_resource_response(
            &format!("ui://circuit-break/{}", uuid::Uuid::new_v4()),
            "text/html",
            &html,
            "ui",
            "circuitBreak",
            Some(&warning_message),
        ))
    }

    fn resume_circuit_break(&self, args: Value) -> Result<MCPResult, String> {
        let tool_name = args
            .get("toolName")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let repetition_count = args
            .get("repetitionCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let text = format!(
            "🔄 Execution Resumed by User\n\n⚠️ IMPORTANT: You have called \"{}\" {} times with the same parameters.\n\nThis suggests your current approach is not working. Please:\n1. Analyze why the previous attempts failed\n2. Try a DIFFERENT approach or tool\n3. If the error persists, inform the user about the limitation\n\nDo NOT repeat the same tool call again.",
            tool_name, repetition_count
        );

        Ok(MCPResult {
            content: Some(vec![MCPContent::Text {
                text,
                is_error: None,
            }]),
            structured_content: None,
            is_error: Some(false),
        })
    }

    fn present_interactive(&self, args: Value) -> Result<MCPResult, String> {
        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => return Ok(missing_param_error("content", ToolGroup::UI)),
        };

        let format = args
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");
        let title = args.get("title").and_then(|v| v.as_str());
        let interaction = args.get("interaction");

        let is_markdown = !matches!(format, "html");
        let content_json = serde_json::to_string(content)
            .unwrap_or_else(|_| "\"\"".to_string())
            .replace("</", "<\\/");

        let message_id = uuid::Uuid::new_v4().to_string();

        let mut data = json!({
            "isMarkdown": is_markdown,
            "contentJson": content_json,
            "messageId": message_id,
        });

        if !is_markdown {
            data.as_object_mut()
                .unwrap()
                .insert("content".to_string(), json!(content));
        }

        if let Some(t) = title {
            data.as_object_mut()
                .unwrap()
                .insert("title".to_string(), json!(t));
        }

        if let Some(inter) = interaction {
            let inter_obj = match inter.as_object() {
                Some(_) => inter.clone(),
                None => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        "interaction must be an object",
                        ToolGroup::UI,
                    )
                    .with_guidance(vec![
                        "Provide interaction as an object with type and prompt".to_string(),
                        "Example: {\"interaction\": {\"type\": \"text\", \"prompt\": \"What next?\"}}".to_string(),
                    ])
                    .to_mcp_result());
                }
            };
            let type_ = match inter.get("type").and_then(|v| v.as_str()) {
                Some(value) => value,
                None => return Ok(missing_param_error("interaction.type", ToolGroup::UI)),
            };
            let prompt = match inter.get("prompt").and_then(|v| v.as_str()) {
                Some(value) => value,
                None => return Ok(missing_param_error("interaction.prompt", ToolGroup::UI)),
            };

            if !["text", "select", "multiselect"].contains(&type_) {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!(
                        "Invalid interaction type '{}'. Supported types: text, select, multiselect",
                        type_
                    ),
                    ToolGroup::UI,
                )
                .with_guidance(vec![
                    "Use 'text' for free-form input".to_string(),
                    "Use 'select' for a single choice".to_string(),
                    "Use 'multiselect' for multiple choices".to_string(),
                ])
                .to_mcp_result());
            }

            data.as_object_mut()
                .unwrap()
                .insert("optionsJson".to_string(), json!("[]"));

            if type_ == "select" || type_ == "multiselect" {
                let options = match inter.get("options") {
                    Some(v) => v,
                    None => return Ok(missing_param_error("interaction.options", ToolGroup::UI)),
                };

                let options_array = match options.as_array() {
                    Some(arr) => arr,
                    None => {
                        return Ok(guided_error(
                            ErrorCategory::InvalidInput,
                            "interaction.options must be an array of strings",
                            ToolGroup::UI,
                        )
                        .to_mcp_result());
                    }
                };

                let mut options_vec = Vec::with_capacity(options_array.len());
                for option in options_array {
                    match option.as_str() {
                        Some(value) => options_vec.push(value.to_string()),
                        None => {
                            return Ok(guided_error(
                                ErrorCategory::InvalidInput,
                                "interaction.options must contain only strings",
                                ToolGroup::UI,
                            )
                            .with_guidance(vec![
                                "Replace numbers or objects with display strings".to_string(),
                                "Example: [\"Approve\", \"Reject\"]".to_string(),
                            ])
                            .to_mcp_result());
                        }
                    }
                }

                let options_json =
                    serde_json::to_string(&options_vec).unwrap_or_else(|_| "[]".to_string());

                data.as_object_mut()
                    .unwrap()
                    .insert("optionsJson".to_string(), json!(options_json));

                let mut options_html = String::new();
                for (i, opt) in options_vec.iter().enumerate() {
                    let input_type = if type_ == "multiselect" {
                        "checkbox"
                    } else {
                        "radio"
                    };
                    options_html.push_str(&format!(
                        r#"<label class="option-item">
                            <input type="{}" name="option" value="{}" class="option-input">
                            <span class="option-label">{}</span>
                           </label>"#,
                        input_type,
                        i,
                        html_escape::encode_text(opt)
                    ));
                }
                data.as_object_mut()
                    .unwrap()
                    .insert("optionsHtml".to_string(), json!(options_html));
            }

            data.as_object_mut()
                .unwrap()
                .insert("interaction".to_string(), inter_obj);
            data.as_object_mut()
                .unwrap()
                .insert("interactionPrompt".to_string(), json!(prompt));
            data.as_object_mut()
                .unwrap()
                .insert("isText".to_string(), json!(type_ == "text"));
        }

        let handlebars = self.handlebars.lock().unwrap();
        let html = match handlebars.render("present-interactive", &data) {
            Ok(h) => h,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::OperationFailed,
                    format!("Failed to render interactive content: {}", e),
                    ToolGroup::UI,
                )
                .to_mcp_result());
            }
        };

        let mut summary_lines = vec![format!(
            "Interactive content rendered: {}",
            title.unwrap_or("Report & Prompt")
        )];

        if let Some(excerpt) = summarize_interactive_content(content, 180) {
            summary_lines.push(format!("Summary: {}", excerpt));
        }

        if let Some(inter) = interaction {
            if let Some(prompt) = inter.get("prompt").and_then(|v| v.as_str()) {
                summary_lines.push(format!("User response required: {}", prompt));
            }

            if let Some(type_) = inter.get("type").and_then(|v| v.as_str()) {
                summary_lines.push(format!("Interaction type: {}", type_));
            }

            summary_lines
                .push("Workflow paused until the user responds via the rendered UI.".to_string());
        }

        let summary = summary_lines.join("\n");

        Ok(crate::mcp::builtin::utils::create_resource_response(
            &format!("ui://interactive/{}", message_id),
            "text/html",
            &html,
            "ui",
            "presentInteractive",
            Some(summary.as_str()),
        ))
    }
}

pub const NAME: &str = "ui";

#[async_trait]
impl BuiltinMCPServer for UiServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "UI Tools for user interaction"
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        ServiceContext {
            context_prompt: String::new(),
            structured_state: None,
        }
    }

    fn tools(&self) -> Vec<MCPTool> {
        tools::all_tools()
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        match tool_name {
            "getUserAnswer" => self.get_user_answer(args),
            "visualizeData" => self.visualize_data(args),
            "circuitBreak" => self.circuit_break(args),
            "resumeCircuitBreak" => self.resume_circuit_break(args),
            "presentInteractive" => self.present_interactive(args),
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }
}
