use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{MCPContent, MCPResult};
use crate::mcp::MCPTool;
use async_trait::async_trait;
use handlebars::Handlebars;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

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
        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or("Missing prompt")?;
        let type_ = args
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or("Missing type")?;

        let message_id = uuid::Uuid::new_v4().to_string();

        let mut data = json!({
            "prompt": prompt,
            "messageId": message_id,
        });

        let template_name = match type_ {
            "text" => "text-prompt",
            "select" | "multiselect" => {
                let options = args
                    .get("options")
                    .ok_or("Missing options for select/multiselect")?;
                data.as_object_mut()
                    .unwrap()
                    .insert("optionsJson".to_string(), json!(options));

                // We also need optionsHtml for the template
                let options_array = options.as_array().ok_or("Options must be an array")?;
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
            _ => return Err(format!("Unknown prompt type: {}", type_)),
        };

        let handlebars = self.handlebars.lock().unwrap();
        let html = handlebars
            .render(template_name, &data)
            .map_err(|e| e.to_string())?;

        Ok(MCPResult {
            content: Some(vec![MCPContent::Resource {
                resource: json!({
                    "uri": format!("ui://prompt/{}", message_id),
                    "mimeType": "text/html",
                    "text": html,
                }),
            }]),
            structured_content: None,
            is_error: Some(false),
        })
    }

    fn reply_prompt(&self, args: Value) -> Result<MCPResult, String> {
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
        let message = args
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or("Missing message")?;
        let resume_instruction = args
            .get("resumeInstruction")
            .and_then(|v| v.as_str())
            .ok_or("Missing resumeInstruction")?;

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
        let html = handlebars
            .render("wait", &data)
            .map_err(|e| e.to_string())?;

        Ok(MCPResult {
            content: Some(vec![MCPContent::Resource {
                resource: json!({
                    "uri": format!("ui://wait/{}", chrono::Utc::now().timestamp_millis()),
                    "mimeType": "text/html",
                    "text": html,
                }),
            }]),
            structured_content: None,
            is_error: Some(false),
        })
    }

    fn resume_from_wait(&self, _args: Value) -> Result<MCPResult, String> {
        Ok(MCPResult {
            content: Some(vec![MCPContent::Text {
                text: "User resumed execution.".to_string(),
            }]),
            structured_content: None,
            is_error: Some(false),
        })
    }

    fn visualize_data(&self, args: Value) -> Result<MCPResult, String> {
        let type_ = args
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or("Missing type")?;
        let data_points = args
            .get("data")
            .and_then(|v| v.as_array())
            .ok_or("Missing data")?;

        if data_points.is_empty() {
            return Err("Data array cannot be empty".to_string());
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
                let html = handlebars
                    .render("bar-chart", &template_data)
                    .map_err(|e| e.to_string())?;

                Ok(MCPResult {
                    content: Some(vec![MCPContent::Resource {
                        resource: json!({
                            "uri": format!("ui://chart/{}", uuid::Uuid::new_v4()),
                            "mimeType": "text/html",
                            "text": html,
                        }),
                    }]),
                    structured_content: None,
                    is_error: Some(false),
                })
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
                let html = handlebars
                    .render("line-chart", &template_data)
                    .map_err(|e| e.to_string())?;

                Ok(MCPResult {
                    content: Some(vec![MCPContent::Resource {
                        resource: json!({
                            "uri": format!("ui://chart/{}", uuid::Uuid::new_v4()),
                            "mimeType": "text/html",
                            "text": html,
                        }),
                    }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            _ => Err(format!("Unknown chart type: {}", type_)),
        }
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

    fn tools(&self) -> Vec<MCPTool> {
        vec![
            MCPTool {
                name: "prompt_user".to_string(),
                description: "Display an interactive prompt to the user (text input, select, or multiselect). Use this to gather user input interactively.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "The question or instruction to show the user"
                        },
                        "type": {
                            "type": "string",
                            "enum": ["text", "select", "multiselect"],
                            "description": "Type of prompt UI to display"
                        },
                        "options": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Options for select/multiselect (required for those types)"
                        }
                    },
                    "required": ["prompt", "type"]
                })).unwrap(),
                output_schema: None,
                title: Some("Prompt User".to_string()),
                annotations: None,
            },
            MCPTool {
                name: "reply_prompt".to_string(),
                description: "Receive user response from prompt UI (automatically called by UI action)".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "messageId": {
                            "type": "string",
                            "description": "ID of the prompt being replied to"
                        },
                        "answer": {
                            "type": "string", // Or array, but schema says string/null/array. We use string for simplicity in schema or "any"
                            "description": "User answer"
                        },
                        "cancelled": {
                            "type": "boolean",
                            "description": "Whether the user cancelled the prompt"
                        }
                    },
                    "required": ["messageId"]
                })).unwrap(),
                output_schema: None,
                title: Some("Reply Prompt".to_string()),
                annotations: None,
            },
            MCPTool {
                name: "visualize_data".to_string(),
                description: "Create a simple data visualization (bar or line chart).".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "type": {
                            "type": "string",
                            "enum": ["bar", "line"],
                            "description": "Type of chart to create"
                        },
                        "data": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "label": { "type": "string" },
                                    "value": { "type": "number" }
                                },
                                "required": ["label", "value"]
                            },
                            "description": "Data points"
                        }
                    },
                    "required": ["type", "data"]
                })).unwrap(),
                output_schema: None,
                title: Some("Visualize Data".to_string()),
                annotations: None,
            },
            MCPTool {
                name: "wait_for_user_resume".to_string(),
                description: "Display wait UI with continue button".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "message": {
                            "type": "string",
                            "description": "Message to display"
                        },
                        "resumeInstruction": {
                            "type": "string",
                            "description": "Instruction for resuming"
                        }
                    },
                    "required": ["message", "resumeInstruction"]
                })).unwrap(),
                output_schema: None,
                title: Some("Wait For User Resume".to_string()),
                annotations: None,
            },
            MCPTool {
                name: "resume_from_wait".to_string(),
                description: "Resume from wait state".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "resumeInstruction": { "type": "string" },
                        "startedAt": { "type": "number" }
                    },
                    "required": ["resumeInstruction"]
                })).unwrap(),
                output_schema: None,
                title: Some("Resume From Wait".to_string()),
                annotations: None,
            },
        ]
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResult, String> {
        match tool_name {
            "prompt_user" | "builtin_ui__prompt_user" => self.prompt_user(args),
            "reply_prompt" | "builtin_ui__reply_prompt" => self.reply_prompt(args),
            "visualize_data" | "builtin_ui__visualize_data" => self.visualize_data(args),
            "wait_for_user_resume" | "builtin_ui__wait_for_user_resume" => {
                self.wait_for_user_resume(args)
            }
            "resume_from_wait" | "builtin_ui__resume_from_wait" => self.resume_from_wait(args),
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }
}
