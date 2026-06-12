use crate::state::{get_message_repository, get_session_repository};
use crate::repositories::session_repository::{SessionRepository, SessionStatus};
use crate::repositories::message_repository::MessageRepository;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use tauri::command;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetFilter {
    pub min_turns: Option<u32>,
    pub max_turns: Option<u32>,
    pub exclude_errors: Option<bool>,
    pub exclude_short: Option<bool>,
    pub min_tokens: Option<u32>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    LlamaFactory,
    Alpaca,
    ShareGPT,
    OpenAIJSONL,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub success: bool,
    pub session_count: usize,
    pub message_count: usize,
    pub output_path: String,
}

#[derive(Debug, Serialize)]
struct ShareGPTMessage {
    from: String,
    value: String,
}

#[derive(Debug, Serialize)]
struct ShareGPTConversation {
    conversations: Vec<ShareGPTMessage>,
}

#[derive(Debug, Serialize)]
struct AlpacaItem {
    instruction: String,
    input: String,
    output: String,
}

#[derive(Debug, Serialize)]
struct OpenAIJSONLItem {
    messages: Vec<OpenAIJSONLMessage>,
}

#[derive(Debug, Serialize)]
struct OpenAIJSONLMessage {
    role: String,
    content: String,
}

#[command]
pub async fn export_dataset(
    session_ids: Option<Vec<String>>,
    format: ExportFormat,
    output_path: String,
    filters: Option<DatasetFilter>,
) -> Result<ExportResult, String> {
    let session_repo = get_session_repository();
    let message_repo = get_message_repository();

    let all_sessions = session_repo
        .get_all_sessions()
        .await
        .map_err(|e| format!("Failed to get all sessions: {}", e))?;

    let filter = filters.unwrap_or(DatasetFilter {
        min_turns: None,
        max_turns: None,
        exclude_errors: None,
        exclude_short: None,
        min_tokens: None,
    });

    let mut sessions_to_export = Vec::new();
    let target_ids = session_ids.unwrap_or_default();

    for s in all_sessions {
        if !target_ids.is_empty() && !target_ids.contains(&s.id) {
            continue;
        }

        if let Some(true) = filter.exclude_errors {
            if s.status == SessionStatus::Error {
                continue;
            }
        }

        sessions_to_export.push(s);
    }

    let mut final_conversations = Vec::new();
    let mut final_alpaca = Vec::new();
    let mut final_openai_jsonl = Vec::new();

    let mut exported_sessions = 0;
    let mut exported_messages = 0;

    for s in &sessions_to_export {
        let messages = message_repo
            .get_messages_by_session(&s.id, 1000)
            .await
            .map_err(|e| format!("Failed to get messages for session {}: {}", s.id, e))?;

        if messages.is_empty() {
            continue;
        }

        let turns = messages.iter().filter(|m| m.role == "user" || m.role == "assistant").count() as u32;

        if let Some(min) = filter.min_turns {
            if turns < min {
                continue;
            }
        }
        if let Some(max) = filter.max_turns {
            if turns > max {
                continue;
            }
        }
        if let Some(true) = filter.exclude_short {
            if turns < 2 {
                continue;
            }
        }

        let total_chars: usize = messages.iter().map(|m| {
            m.content.iter().map(|c| extract_message_text(c)).collect::<Vec<String>>().join(" ").len()
        }).sum();

        let est_tokens = (total_chars / 4) as u32;
        if let Some(min) = filter.min_tokens {
            if est_tokens < min {
                continue;
            }
        }

        exported_sessions += 1;
        exported_messages += messages.len();

        match format {
            ExportFormat::LlamaFactory | ExportFormat::ShareGPT => {
                let mut conv = Vec::new();
                for m in &messages {
                    let from = match m.role.as_str() {
                        "user" => "human".to_string(),
                        "assistant" => "gpt".to_string(),
                        _ => "system".to_string(),
                    };
                    let text = m.content.iter().map(|c| extract_message_text(c)).collect::<Vec<String>>().join("\n");
                    conv.push(ShareGPTMessage { from, value: text });
                }
                final_conversations.push(ShareGPTConversation { conversations: conv });
            }
            ExportFormat::Alpaca => {
                let mut user_text = String::new();
                for m in &messages {
                    let text = m.content.iter().map(|c| extract_message_text(c)).collect::<Vec<String>>().join("\n");
                    if m.role == "user" {
                        user_text = text;
                    } else if m.role == "assistant" && !user_text.is_empty() {
                        final_alpaca.push(AlpacaItem {
                            instruction: "Respond to the user prompt below.".to_string(),
                            input: user_text.clone(),
                            output: text,
                        });
                        user_text.clear();
                    }
                }
            }
            ExportFormat::OpenAIJSONL => {
                let mut conv = Vec::new();
                for m in &messages {
                    let role = match m.role.as_str() {
                        "user" => "user".to_string(),
                        "assistant" => "assistant".to_string(),
                        _ => "system".to_string(),
                    };
                    let content = m.content.iter().map(|c| extract_message_text(c)).collect::<Vec<String>>().join("\n");
                    conv.push(OpenAIJSONLMessage { role, content });
                }
                final_openai_jsonl.push(OpenAIJSONLItem { messages: conv });
            }
        }
    }

    let mut file = File::create(&output_path)
        .map_err(|e| format!("Failed to create output file: {}", e))?;

    match format {
        ExportFormat::LlamaFactory | ExportFormat::ShareGPT => {
            let json_str = serde_json::to_string_pretty(&final_conversations)
                .map_err(|e| format!("Failed to serialize ShareGPT dataset: {}", e))?;
            file.write_all(json_str.as_bytes())
                .map_err(|e| format!("Failed to write dataset to file: {}", e))?;
        }
        ExportFormat::Alpaca => {
            let json_str = serde_json::to_string_pretty(&final_alpaca)
                .map_err(|e| format!("Failed to serialize Alpaca dataset: {}", e))?;
            file.write_all(json_str.as_bytes())
                .map_err(|e| format!("Failed to write dataset to file: {}", e))?;
        }
        ExportFormat::OpenAIJSONL => {
            for item in final_openai_jsonl {
                let json_line = serde_json::to_string(&item)
                    .map_err(|e| format!("Failed to serialize OpenAI JSONL item: {}", e))?;
                writeln!(file, "{}", json_line)
                    .map_err(|e| format!("Failed to write JSONL line to file: {}", e))?;
            }
        }
    }

    Ok(ExportResult {
        success: true,
        session_count: exported_sessions,
        message_count: exported_messages,
        output_path,
    })
}

fn extract_message_text(content: &crate::mcp::types::MCPContent) -> String {
    match content {
        crate::mcp::types::MCPContent::Text { text, .. } => text.clone(),
        crate::mcp::types::MCPContent::Thinking { thinking, .. } => format!("<thinking>\n{}\n</thinking>", thinking),
        crate::mcp::types::MCPContent::ToolCall { name, arguments, .. } => {
            format!("<tool_call name=\"{}\" arguments=\"{}\"/>", name, arguments)
        }
        _ => String::new(),
    }
}
