use tauri_mcp_agent_lib::agent::{tools::runtime_allowed_builtin_service_aliases, AgentConfig};
use tauri_mcp_agent_lib::mcp::builtin::agent::tools as agent_tools;
use tauri_mcp_agent_lib::mcp::builtin::service_id::{
    BuiltinServiceId, CORE_BUILTIN_SERVICE_ALIASES,
};

#[test]
fn runtime_builtin_capabilities_keep_core_services_even_with_restricted_config() {
    let config = AgentConfig {
        allowed_built_in_service_aliases: Some(vec![
            BuiltinServiceId::Workspace,
            BuiltinServiceId::Planning,
        ]),
        ..AgentConfig::default()
    };

    let effective = runtime_allowed_builtin_service_aliases(&config);

    assert!(effective.contains(&"planning".to_string()));
    assert!(effective.contains(&"workspace".to_string()));
    assert!(effective.contains(&"scratchpad".to_string()));
    assert!(effective.contains(&"playbook".to_string()));
    assert!(effective.contains(&"attachments".to_string()));
    assert!(!effective.contains(&"browser".to_string()));
}

#[test]
fn empty_allowed_builtin_list_enables_core_services_only() {
    // createAgent persists [] when builtinCapabilities is omitted (#1615).
    let config = AgentConfig {
        allowed_built_in_service_aliases: Some(vec![]),
        ..AgentConfig::default()
    };

    let effective = runtime_allowed_builtin_service_aliases(&config);

    for alias in CORE_BUILTIN_SERVICE_ALIASES {
        assert!(
            effective.contains(&alias.to_string()),
            "core alias `{alias}` must be enabled when optional list is empty"
        );
    }
    assert!(!effective.contains(&"planning".to_string()));
    assert!(!effective.contains(&"browser".to_string()));
    assert!(!effective.contains(&"knowledge".to_string()));
    assert!(!effective.contains(&"history".to_string()));
    assert!(!effective.contains(&"media".to_string()));
    assert!(!effective.contains(&"bootstrap".to_string()));
}

#[test]
fn create_agent_schema_documents_core_only_default_when_capabilities_omitted() {
    let tool = agent_tools::all_tools()
        .into_iter()
        .find(|tool| tool.name == "createAgent")
        .expect("createAgent tool");

    let description = match &tool.input_schema.schema_type {
        tauri_mcp_agent_lib::mcp::schema::JSONSchemaType::Object {
            properties: Some(properties),
            ..
        } => properties
            .get("builtinCapabilities")
            .and_then(|schema| schema.description.as_deref())
            .unwrap_or_default(),
        _ => panic!("createAgent input_schema must be an object"),
    };

    assert!(
        description.contains("only core services")
            || description.contains("only the always-on core"),
        "createAgent schema must document omit → core-only default: {description}"
    );
    assert!(
        !description.contains("all optional builtin services stay enabled"),
        "createAgent schema must not advertise the old all-optional default: {description}"
    );
}
