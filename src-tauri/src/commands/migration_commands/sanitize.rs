use crate::entity::{assistant, mcp_server};

use super::models::{AssistantRecord, McpServerRecord};

pub(crate) fn mask_sensitive_json(val: &mut serde_json::Value) {
    match val {
        serde_json::Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                let k_lower = k.to_lowercase();
                if k_lower.contains("key")
                    || k_lower.contains("token")
                    || k_lower.contains("password")
                    || k_lower.contains("secret")
                    || k_lower.contains("auth")
                    || k_lower.contains("credential")
                {
                    if let serde_json::Value::String(s) = v {
                        *s = "REDACTED".to_string();
                    } else {
                        *v = serde_json::Value::Null;
                    }
                } else {
                    mask_sensitive_json(v);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                mask_sensitive_json(item);
            }
        }
        _ => {}
    }
}

pub(crate) fn clean_mcp_servers(
    records: &[mcp_server::Model],
    include_sensitive: bool,
) -> Vec<McpServerRecord> {
    let mut cleaned = Vec::new();
    for r in records {
        let mut config_str = r.config.clone();
        if !include_sensitive {
            if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&r.config) {
                mask_sensitive_json(&mut val);
                config_str = serde_json::to_string(&val).unwrap_or(r.config.clone());
            }
        }
        cleaned.push(McpServerRecord {
            id: r.id.clone(),
            name: r.name.clone(),
            config: config_str,
            tool_count: r.tool_count,
            cached_tools: r.cached_tools.clone(),
            verification_status: r.verification_status.clone(),
            last_verification_error: r.last_verification_error.clone(),
            created_at: r.created_at,
            updated_at: r.updated_at,
        });
    }
    cleaned
}

pub(crate) fn clean_assistants(
    records: &[assistant::Model],
    include_sensitive: bool,
) -> Vec<AssistantRecord> {
    let mut cleaned = Vec::new();
    for r in records {
        let mut config_str = r.config.clone();
        if !include_sensitive {
            if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&r.config) {
                mask_sensitive_json(&mut val);
                config_str = serde_json::to_string(&val).unwrap_or(r.config.clone());
            }
        }
        cleaned.push(AssistantRecord {
            id: r.id.clone(),
            name: r.name.clone(),
            config: config_str,
            created_at: r.created_at,
            updated_at: r.updated_at,
        });
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exported_servers_never_carry_stored_secrets() {
        let model = mcp_server::Model {
            id: "mcp-1".to_string(),
            name: "exa".to_string(),
            config: r#"{"transport":{"type":"http","url":"https://mcp.exa.ai/mcp?exaApiKey={{secret:EXA_API_KEY}}"}}"#
                .to_string(),
            tool_count: None,
            cached_tools: None,
            verification_status: None,
            last_verification_error: None,
            secret_names: Some(r#"["EXA_API_KEY"]"#.to_string()),
            encrypted_secrets: Some("sealed-value-map".to_string()),
            created_at: 1,
            updated_at: 2,
        };

        for include_sensitive in [false, true] {
            let exported =
                serde_json::to_string(&clean_mcp_servers(&[model.clone()], include_sensitive))
                    .unwrap();
            assert!(!exported.contains("sealed-value-map"), "{exported}");
            assert!(!exported.contains("encrypted_secrets"), "{exported}");
        }
    }
}
