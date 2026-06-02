use tauri_mcp_agent_lib::agent::{tools::runtime_allowed_builtin_service_aliases, AgentConfig};
use tauri_mcp_agent_lib::mcp::builtin::service_id::BuiltinServiceId;

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
