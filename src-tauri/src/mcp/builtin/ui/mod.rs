use crate::mcp::builtin::error_guidance::{
    invalid_input_error, missing_param_error, operation_failed_error, ToolGroup,
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
const SELECT_PROMPT_TEMPLATE: &str = include_str!("templates/select-prompt.hbs");
const TEXT_PROMPT_TEMPLATE: &str = include_str!("templates/text-prompt.hbs");
const WAIT_TEMPLATE: &str = include_str!("templates/wait.hbs");

#[derive(Debug)]
pub struct UiServer {
    handlebars: Arc<Mutex<Handlebars<'static>>>,
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
            .register_template_string("select-prompt", SELECT_PROMPT_TEMPLATE)
            .unwrap();
        handlebars
            .register_template_string("text-prompt", TEXT_PROMPT_TEMPLATE)
            .unwrap();
        handlebars
            .register_template_string("wait", WAIT_TEMPLATE)
            .unwrap();

        Self {
            handlebars: Arc::new(Mutex::new(handlebars)),
        }
    }

    fn prompt_user(&self, args: Value) -> Result<MCPResult, String> {
        let prompt = match args.get("prompt").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => return Ok(missing_param_error("prompt", ToolGroup::UI)),
        };
        let type_ = match args.get("type").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => return Ok(missing_param_error("type", ToolGroup::UI)),
        };

        // Validate type
        if !["text", "select", "multiselect"].contains(&type_) {
            return Ok(invalid_input_error(
                &format!(
                    "Invalid type '{}'. Must be one of: text, select, multiselect",
                    type_
                ),
                ToolGroup::UI,
            ));
        }

        let message_id = uuid::Uuid::new_v4().to_string();

        let mut data = json!({
            "prompt": prompt,
            "messageId": message_id,
        });

        let template_name = match type_ {
            "text" => "text-prompt",
            "select" | "multiselect" => {
                let options = match args.get("options") {
                    Some(v) => v,
                    None => {
                        return Ok(missing_param_error("options", ToolGroup::UI));
                    }
                };

                let options_array = match options.as_array() {
                    Some(arr) => arr,
                    None => {
                        return Ok(invalid_input_error(
                            "options must be an array of strings",
                            ToolGroup::UI,
                        ));
                    }
                };

                // Convert options array to Vec<String> for JSON
                let options: Vec<String> = options_array
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();

                // Serialize to JSON string for JavaScript consumption
                let options_json =
                    serde_json::to_string(&options).unwrap_or_else(|_| "[]".to_string());

                // Insert as string so Handlebars renders it as valid JavaScript array literal
                data.as_object_mut()
                    .unwrap()
                    .insert("optionsJson".to_string(), json!(options_json));

                // We also need optionsHtml for the template
                let mut options_html = String::new();
                for (i, opt) in options_array.iter().enumerate() {
                    let opt_str = opt.as_str().unwrap_or_default();
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
                        html_escape::encode_text(opt_str)
                    ));
                }
                data.as_object_mut()
                    .unwrap()
                    .insert("optionsHtml".to_string(), json!(options_html));
                data.as_object_mut()
                    .unwrap()
                    .insert("multiselect".to_string(), json!(type_ == "multiselect"));

                "select-prompt"
            }
            _ => {
                return Ok(invalid_input_error(
                    &format!(
                        "Invalid type '{}'. Must be one of: text, select, multiselect",
                        type_
                    ),
                    ToolGroup::UI,
                ))
            }
        };

        let handlebars = self.handlebars.lock().unwrap();
        let html = match handlebars.render(template_name, &data) {
            Ok(h) => h,
            Err(e) => {
                return Ok(operation_failed_error(
                    "promptUser",
                    &format!("Template rendering failed: {}", e),
                    vec![
                        "Verify template data format is correct".to_string(),
                        "Check that all required template variables are provided".to_string(),
                    ],
                    ToolGroup::UI,
                ));
            }
        };

        Ok(crate::mcp::builtin::utils::create_resource_response(
            &format!("ui://prompt/{}", message_id),
            "text/html",
            &html,
            "ui",
            "promptUser",
            None,
        ))
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
            return Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: "User cancelled the prompt.".to_string(),
                }]),
                structured_content: None,
                is_error: Some(true),
            });
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
            }]),
            structured_content: None,
            is_error: Some(false),
        })
    }

    fn wait_for_user_resume(&self, args: Value) -> Result<MCPResult, String> {
        let message = match args.get("message").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => return Ok(missing_param_error("message", ToolGroup::UI)),
        };
        let resume_instruction = match args.get("resumeInstruction").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => return Ok(missing_param_error("resumeInstruction", ToolGroup::UI)),
        };

        let context_json = json!({
            "resumeInstruction": resume_instruction,
            "startedAt": chrono::Utc::now().timestamp_millis(),
        });

        let data = json!({
            "message": message,
            "contextJson": context_json.to_string(),
            "contextHtml": html_escape::encode_text(&context_json.to_string()),
        });

        let handlebars = self.handlebars.lock().unwrap();
        let html = match handlebars.render("wait", &data) {
            Ok(h) => h,
            Err(e) => {
                return Ok(operation_failed_error(
                    "waitForUserResume",
                    &format!("Template rendering failed: {}", e),
                    vec![
                        "Verify template data format is correct".to_string(),
                        "Check that message and resumeInstruction are provided".to_string(),
                    ],
                    ToolGroup::UI,
                ));
            }
        };

        Ok(crate::mcp::builtin::utils::create_resource_response(
            &format!("ui://wait/{}", chrono::Utc::now().timestamp_millis()),
            "text/html",
            &html,
            "ui",
            "waitForUserResume",
            None,
        ))
    }

    fn resume_from_wait(&self, args: Value) -> Result<MCPResult, String> {
        let _resume_instruction = match args.get("resumeInstruction").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => return Ok(missing_param_error("resumeInstruction", ToolGroup::UI)),
        };

        Ok(MCPResult {
            content: Some(vec![MCPContent::Text {
                text: "User resumed execution.".to_string(),
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
            return Ok(invalid_input_error(
                &format!("Invalid type '{}'. Must be one of: bar, line", type_),
                ToolGroup::UI,
            ));
        }

        let data_points = match args.get("data").and_then(|v| v.as_array()) {
            Some(arr) => arr,
            None => return Ok(missing_param_error("data", ToolGroup::UI)),
        };

        if data_points.is_empty() {
            return Ok(invalid_input_error(
                "Data array cannot be empty",
                ToolGroup::UI,
            ));
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
                        return Ok(operation_failed_error(
                            "visualizeData",
                            &format!("Template rendering failed: {}", e),
                            vec![
                                "Verify data format is correct".to_string(),
                                "Ensure all data points have label and value".to_string(),
                            ],
                            ToolGroup::UI,
                        ));
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
                        return Ok(operation_failed_error(
                            "visualizeData",
                            &format!("Template rendering failed: {}", e),
                            vec![
                                "Verify data format is correct".to_string(),
                                "Ensure all data points have label and value".to_string(),
                            ],
                            ToolGroup::UI,
                        ));
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
            _ => Ok(invalid_input_error(
                &format!("Invalid type '{}'. Must be one of: bar, line", type_),
                ToolGroup::UI,
            )),
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
                return Ok(operation_failed_error(
                    "circuitBreak",
                    &format!("Template rendering failed: {}", e),
                    vec!["Verify template data".to_string()],
                    ToolGroup::UI,
                ));
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
            content: Some(vec![MCPContent::Text { text }]),
            structured_content: None,
            is_error: Some(false),
        })
    }
}

#[async_trait]
impl BuiltinMCPServer for UiServer {
    fn name(&self) -> &str {
        "ui"
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
            "promptUser" | "builtin_ui__promptUser" => self.prompt_user(args),
            "getUserAnswer" | "builtin_ui__getUserAnswer" => self.get_user_answer(args),
            "visualizeData" | "builtin_ui__visualizeData" => self.visualize_data(args),
            "waitForUserResume" | "builtin_ui__waitForUserResume" => {
                self.wait_for_user_resume(args)
            }
            "resumeFromWait" | "builtin_ui__resumeFromWait" => self.resume_from_wait(args),
            "circuitBreak" | "builtin_ui__circuitBreak" => self.circuit_break(args),
            "resumeCircuitBreak" | "builtin_ui__resumeCircuitBreak" => {
                self.resume_circuit_break(args)
            }
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }
}
