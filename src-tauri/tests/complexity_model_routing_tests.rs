//! Complexity-based model routing resolve contract (pure + schema).

use serde_json::json;
use tauri_mcp_agent_lib::mcp::builtin::agent::tools::all_tools;
use tauri_mcp_agent_lib::mcp::builtin::agent::utils::{
    parse_complexity, resolve_from_mapping, COMPLEXITY_LEVELS,
};
use tauri_mcp_agent_lib::mcp::schema::JSONSchemaType;

#[test]
fn parse_complexity_rejects_unknown() {
    assert!(parse_complexity("medium").is_err());
    assert_eq!(parse_complexity("high").unwrap(), "high");
}

#[test]
fn resolve_none_when_mapping_empty_keeps_default_chain() {
    for level in COMPLEXITY_LEVELS {
        assert!(
            resolve_from_mapping(level, None).unwrap().is_none(),
            "unset mapping must not force a model override"
        );
    }
}

#[test]
fn resolve_uses_independent_level_overrides() {
    let mapping = json!({
        "low": { "model": "gpt-4o-mini", "provider": "openai" },
        "high": { "model": "claude-opus", "provider": "anthropic" }
    });

    let low = resolve_from_mapping("low", Some(&mapping))
        .unwrap()
        .expect("low override");
    assert_eq!(low.model, "gpt-4o-mini");
    assert_eq!(low.provider, "openai");

    assert!(resolve_from_mapping("normal", Some(&mapping))
        .unwrap()
        .is_none());

    let high = resolve_from_mapping("high", Some(&mapping))
        .unwrap()
        .expect("high override");
    assert_eq!(high.model, "claude-opus");
    assert_eq!(high.provider, "anthropic");
}

#[test]
fn spawn_and_message_tools_require_complexity() {
    let tools = all_tools();
    for name in ["spawnSession", "messageToSession"] {
        let tool = tools
            .iter()
            .find(|t| t.name == name)
            .unwrap_or_else(|| panic!("{name} tool missing"));

        let JSONSchemaType::Object {
            properties: Some(properties),
            required: Some(required),
            ..
        } = &tool.input_schema.schema_type
        else {
            panic!("{name} input_schema must be an object with required fields");
        };

        assert!(
            required.iter().any(|k| k == "complexity"),
            "{name} must require complexity"
        );
        let complexity = properties
            .get("complexity")
            .unwrap_or_else(|| panic!("{name} missing complexity prop"));
        let enums = complexity
            .enum_values
            .as_ref()
            .unwrap_or_else(|| panic!("{name} complexity must be an enum"));
        assert_eq!(enums, &vec![json!("low"), json!("normal"), json!("high")]);
    }
}
