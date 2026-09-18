//! Windows-safe integration tests for Playbook session pinning, slots, and prompt interpolation.

use std::collections::HashMap;
use tauri_mcp_agent_lib::entity::playbook;
use tauri_mcp_agent_lib::mcp::builtin::playbook::{
    format_playbook_detailed, interpolate_variables, Playbook, PlaybookStep, TargetSessionMode,
};

#[test]
fn test_playbook_step_with_session_targeting_deserialization() {
    let step_json = serde_json::json!({
        "stepId": "analyze_data",
        "description": "Perform deep data analysis",
        "action": {
            "toolName": "agent__spawnSession",
            "purpose": "Delegate analysis to specialist"
        },
        "targetSession": {
            "mode": "spawn",
            "configId": "analyst-config-123",
            "sessionSlot": "analyst"
        },
        "sessionSlot": "analyst",
        "promptTemplate": "Please analyze the following data: {input_dataset}",
        "requiredData": ["input_dataset"],
        "outputVariable": "analysis_report"
    });

    let step: PlaybookStep = serde_json::from_value(step_json).expect("failed to deserialize step");
    assert_eq!(step.step_id.as_deref(), Some("analyze_data"));
    assert_eq!(step.session_slot.as_deref(), Some("analyst"));
    assert_eq!(
        step.prompt_template.as_deref(),
        Some("Please analyze the following data: {input_dataset}")
    );

    let target = step
        .target_session
        .expect("target_session should be present");
    assert_eq!(target.mode, TargetSessionMode::Spawn);
    assert_eq!(target.config_id.as_deref(), Some("analyst-config-123"));
    assert_eq!(target.session_slot.as_deref(), Some("analyst"));
}

#[test]
fn test_playbook_backward_compatible_from_model_with_legacy_array() {
    let legacy_model = playbook::Model {
        id: "pb-legacy-1".to_string(),
        assistant_id: "ast-1".to_string(),
        goal: "Legacy workflow goal".to_string(),
        initial_command: Some("run legacy".to_string()),
        workflow: serde_json::json!([
            {
                "stepId": "step_1",
                "description": "Do something",
                "action": { "toolName": "workspace__readFile", "purpose": "Read file" },
                "requiredData": [],
                "outputVariable": "file_data"
            }
        ])
        .to_string(),
        success_criteria: None,
        created_at: 1000,
        updated_at: 1000,
        is_bookmarked: false,
    };

    let pb = Playbook::from_model(&legacy_model);
    assert_eq!(pb.id, "pb-legacy-1");
    assert_eq!(pb.workflow.len(), 1);
    assert_eq!(pb.workflow[0].step_id.as_deref(), Some("step_1"));
    assert!(pb.default_target_session.is_none());
    assert!(pb.session_slots.is_none());
}

#[test]
fn test_playbook_from_model_with_structured_session_slots() {
    let structured_model = playbook::Model {
        id: "pb-multi-1".to_string(),
        assistant_id: "ast-2".to_string(),
        goal: "Multi-agent review workflow".to_string(),
        initial_command: Some("review code".to_string()),
        workflow: serde_json::json!({
            "defaultTargetSession": {
                "mode": "self"
            },
            "sessionSlots": {
                "analyst": {
                    "mode": "spawn",
                    "configId": "analyst-template",
                    "reuseAcrossSteps": true
                },
                "reviewer": {
                    "mode": "pin",
                    "sessionId": "pinned-rev-10"
                }
            },
            "steps": [
                {
                    "stepId": "step_fetch",
                    "description": "Fetch code changes",
                    "action": { "toolName": "workspace__readFile", "purpose": "Read source" },
                    "requiredData": [],
                    "outputVariable": "code"
                },
                {
                    "stepId": "step_analyze",
                    "sessionSlot": "analyst",
                    "promptTemplate": "Analyze the following code: {code}",
                    "description": "Deep code analysis",
                    "action": { "toolName": "agent__spawnSession", "purpose": "Spawn analyst" },
                    "requiredData": ["code"],
                    "outputVariable": "analysis"
                }
            ]
        })
        .to_string(),
        success_criteria: None,
        created_at: 2000,
        updated_at: 2000,
        is_bookmarked: true,
    };

    let pb = Playbook::from_model(&structured_model);
    assert_eq!(pb.id, "pb-multi-1");
    assert_eq!(pb.workflow.len(), 2);

    let default_target = pb
        .default_target_session
        .as_ref()
        .expect("default_target should be present");
    assert_eq!(default_target.mode, TargetSessionMode::Self_);

    let slots = pb
        .session_slots
        .as_ref()
        .expect("session_slots should be present");
    assert_eq!(slots.len(), 2);
    let analyst_slot = slots.get("analyst").expect("analyst slot exists");
    assert_eq!(analyst_slot.mode, TargetSessionMode::Spawn);
    assert_eq!(analyst_slot.config_id.as_deref(), Some("analyst-template"));
    assert_eq!(analyst_slot.reuse_across_steps, Some(true));

    let reviewer_slot = slots.get("reviewer").expect("reviewer slot exists");
    assert_eq!(reviewer_slot.mode, TargetSessionMode::Pin);
    assert_eq!(reviewer_slot.session_id.as_deref(), Some("pinned-rev-10"));

    // Check detailed formatting output includes slots & targets
    let detailed = format_playbook_detailed(&pb);
    assert!(detailed.contains("Multi-agent review workflow"));
    assert!(detailed.contains("Default Target Session: Mode=self"));
    assert!(detailed.contains("--- Session Slots ---"));
    assert!(detailed.contains("Slot \"analyst\": Mode=spawn"));
    assert!(detailed.contains("Slot \"reviewer\": Mode=pin"));
    assert!(detailed.contains("Target Session Slot: \"analyst\""));
    assert!(detailed.contains("Prompt Template: \"Analyze the following code: {code}\""));
}

#[test]
fn test_interpolate_variables() {
    let template = "Analyze {dataset} with metric {metric} and send report to {recipient}.";
    let mut vars = HashMap::new();
    vars.insert("dataset".to_string(), "users_2026.parquet".to_string());
    vars.insert("metric".to_string(), "retention_rate".to_string());

    let interpolated = interpolate_variables(template, &vars);
    assert_eq!(
        interpolated,
        "Analyze users_2026.parquet with metric retention_rate and send report to {recipient}."
    );
}

#[test]
fn test_interpolate_variables_substring_safety() {
    let template = "Hello {user}, your user_name is {user_name} and score is {user_score}!";
    let mut vars = HashMap::new();
    vars.insert("user".to_string(), "alice".to_string());
    vars.insert("user_name".to_string(), "Alice Wonderland".to_string());
    vars.insert("user_score".to_string(), "100".to_string());

    let interpolated = interpolate_variables(template, &vars);
    assert_eq!(
        interpolated,
        "Hello alice, your user_name is Alice Wonderland and score is 100!"
    );
}

#[tokio::test]
async fn test_select_playbook_forces_pin_and_dynamic_slot() {
    use sea_orm::{ConnectOptions, ConnectionTrait, Database, EntityTrait, Schema, Set};
    use std::sync::Arc;
    use tauri_mcp_agent_lib::entity::{playbook, session};
    use tauri_mcp_agent_lib::mcp::builtin::playbook::PlaybookServer;
    use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
    use tauri_mcp_agent_lib::repositories::{SqlitePlaybookRepository, SqliteSessionRepository};

    let mut opt =
        ConnectOptions::new("sqlite::file:playbook_pinning_test?mode=memory&cache=shared");
    opt.min_connections(1);
    opt.max_connections(1);
    let db = Database::connect(opt)
        .await
        .expect("Failed to connect to in-memory database");

    let schema = Schema::new(db.get_database_backend());
    let _ = db
        .execute(
            db.get_database_backend()
                .build(&schema.create_table_from_entity(session::Entity)),
        )
        .await;
    let _ = db
        .execute(
            db.get_database_backend()
                .build(&schema.create_table_from_entity(playbook::Entity)),
        )
        .await;

    let db_arc = Arc::new(db);
    tauri_mcp_agent_lib::set_session_repository(SqliteSessionRepository::new((*db_arc).clone()));
    tauri_mcp_agent_lib::set_playbook_repository(SqlitePlaybookRepository::new((*db_arc).clone()));

    let new_session = session::ActiveModel {
        id: Set("pin-test-session".to_string()),
        name: Set(Some("Pin Test Session".to_string())),
        model: Set("gpt-4".to_string()),
        provider: Set("openai".to_string()),
        status: Set("idle".to_string()),
        assistant_id: Set(Some("ast-pin-test".to_string())),
        parent_session_id: Set(None),
        lineage_id: Set(None),
        depth: Set(None),
        max_depth: Set(None),
        max_fanout: Set(None),
        org_id: Set(None),
        org_name: Set(None),
        org_root_session_id: Set(None),
        created_at: Set(0),
        updated_at: Set(0),
        last_viewed_at: Set(None),
        last_message_at: Set(None),
        last_attention_at: Set(None),
        last_attention_reason: Set(None),
        is_bookmarked: Set(false),
        execution_mode: Set("normal".to_string()),
        workspace_override: Set(None),
        workspace_isolation: Set("host".to_string()),
        docker_config_json: Set(None),
        docker_container_name: Set(None),
        docker_host_workspace_path: Set(None),
    };
    session::Entity::insert(new_session)
        .exec(db_arc.as_ref())
        .await
        .expect("Failed to insert session");

    let server = PlaybookServer::new("pin-test-session".to_string(), db_arc.clone())
        .await
        .expect("Failed to create server");

    // 1. Create playbook with defaultTargetSession mode="spawn"
    let create_res = server
        .call_tool(
            "createPlaybook",
            serde_json::json!({
                "goal": "Test Pinning and Slots",
                "defaultTargetSession": {
                    "mode": "spawn",
                    "configId": "some-agent-cfg"
                },
                "sessionSlots": {
                    "known_worker": {
                        "mode": "spawn",
                        "configId": "worker-cfg"
                    }
                },
                "workflow": [
                    {
                        "stepId": "s1",
                        "description": "Step 1",
                        "sessionSlot": "known_worker",
                        "promptTemplate": "Process data from: {data_file}",
                        "action": { "toolName": "tool", "purpose": "purpose" },
                        "outputVariable": "out1"
                    }
                ]
            }),
            None,
        )
        .await
        .expect("createPlaybook failed");
    assert!(!create_res.is_error.unwrap_or(false));

    let created_pb = create_res.structured_content.expect("structured content");
    let pb_id = created_pb["playbook"]["id"]
        .as_str()
        .expect("id")
        .to_string();

    // 2. Call selectPlaybook with targetSessionId (should force mode to Pin),
    //    pinnedSessionSlots with both known and unknown slots,
    //    and variables for automated promptTemplate substitution
    let select_res = server
        .call_tool(
            "selectPlaybook",
            serde_json::json!({
                "id": pb_id,
                "targetSessionId": "runtime-override-sess-999",
                "variables": {
                    "data_file": "2026_q3_sales.csv"
                },
                "pinnedSessionSlots": {
                    "known_worker": "worker-pinned-111",
                    "dynamic_unregistered_slot": "dyn-sess-222"
                }
            }),
            None,
        )
        .await
        .expect("selectPlaybook failed");

    assert!(!select_res.is_error.unwrap_or(false));
    let select_data = select_res.structured_content.expect("structured content");
    let playbook_val = &select_data["playbook"];

    // Verify automated promptTemplate substitution occurred
    assert_eq!(
        playbook_val["workflow"][0]["promptTemplate"],
        "Process data from: 2026_q3_sales.csv"
    );
    assert_eq!(
        select_data["executionContext"]["interpolatedPrompts"]["s1"],
        "Process data from: 2026_q3_sales.csv"
    );

    // Verify defaultTargetSession mode was forced to pin with the target session ID
    assert_eq!(playbook_val["defaultTargetSession"]["mode"], "pin");
    assert_eq!(
        playbook_val["defaultTargetSession"]["sessionId"],
        "runtime-override-sess-999"
    );

    // Verify known_worker slot mode was updated to pin with the pinned session ID
    assert_eq!(playbook_val["sessionSlots"]["known_worker"]["mode"], "pin");
    assert_eq!(
        playbook_val["sessionSlots"]["known_worker"]["sessionId"],
        "worker-pinned-111"
    );

    // Verify dynamic_unregistered_slot was registered with mode: pin
    assert_eq!(
        playbook_val["sessionSlots"]["dynamic_unregistered_slot"]["mode"],
        "pin"
    );
    assert_eq!(
        playbook_val["sessionSlots"]["dynamic_unregistered_slot"]["sessionId"],
        "dyn-sess-222"
    );

    // 3. Test invalid session config returns invalid_input_error instead of silent drop
    let bad_config_res = server
        .call_tool(
            "createPlaybook",
            serde_json::json!({
                "goal": "Bad Config Playbook",
                "defaultTargetSession": { "mode": "invalid_mode_value" },
                "workflow": [
                    {
                        "stepId": "s1",
                        "description": "Step 1",
                        "action": { "toolName": "tool", "purpose": "purpose" },
                        "outputVariable": "out1"
                    }
                ]
            }),
            None,
        )
        .await
        .expect("createPlaybook call failed");
    assert!(bad_config_res.is_error.unwrap_or(false));
}

#[tokio::test]
async fn test_playbook_service_update_preserves_step_config() {
    use sea_orm::{ConnectOptions, ConnectionTrait, Database, EntityTrait, Schema, Set};
    use std::sync::Arc;
    use tauri_mcp_agent_lib::entity::{playbook, session};
    use tauri_mcp_agent_lib::repositories::{SqlitePlaybookRepository, SqliteSessionRepository};
    use tauri_mcp_agent_lib::services::playbook_service::PlaybookService;

    let mut opt =
        ConnectOptions::new("sqlite::file:playbook_service_step_test?mode=memory&cache=shared");
    opt.min_connections(1);
    opt.max_connections(1);
    let db = Database::connect(opt)
        .await
        .expect("Failed to connect to in-memory database");

    let schema = Schema::new(db.get_database_backend());
    let _ = db
        .execute(
            db.get_database_backend()
                .build(&schema.create_table_from_entity(session::Entity)),
        )
        .await;
    let _ = db
        .execute(
            db.get_database_backend()
                .build(&schema.create_table_from_entity(playbook::Entity)),
        )
        .await;

    let db_arc = Arc::new(db);
    let session_repo = SqliteSessionRepository::new((*db_arc).clone());
    let playbook_repo = SqlitePlaybookRepository::new((*db_arc).clone());

    let new_session = session::ActiveModel {
        id: Set("sess-service-test".to_string()),
        name: Set(Some("Test Session".to_string())),
        model: Set("gpt-4".to_string()),
        provider: Set("openai".to_string()),
        status: Set("idle".to_string()),
        assistant_id: Set(Some("ast-service-test".to_string())),
        parent_session_id: Set(None),
        lineage_id: Set(None),
        depth: Set(None),
        max_depth: Set(None),
        max_fanout: Set(None),
        org_id: Set(None),
        org_name: Set(None),
        org_root_session_id: Set(None),
        created_at: Set(0),
        updated_at: Set(0),
        last_viewed_at: Set(None),
        last_message_at: Set(None),
        last_attention_at: Set(None),
        last_attention_reason: Set(None),
        is_bookmarked: Set(false),
        execution_mode: Set("normal".to_string()),
        workspace_override: Set(None),
        workspace_isolation: Set("host".to_string()),
        docker_config_json: Set(None),
        docker_container_name: Set(None),
        docker_host_workspace_path: Set(None),
    };
    session::Entity::insert(new_session)
        .exec(db_arc.as_ref())
        .await
        .expect("Failed to insert session");

    // Create playbook with promptTemplate and sessionSlot on step_1
    let created = PlaybookService::create_playbook(
        &playbook_repo,
        &session_repo,
        "pb-step-preserve-1".to_string(),
        "sess-service-test",
        "Preserve Step Config Goal".to_string(),
        serde_json::json!({
            "defaultTargetSession": { "mode": "self" },
            "sessionSlots": { "worker": { "mode": "spawn" } },
            "steps": [
                {
                    "stepId": "step_1",
                    "description": "Original Step Description",
                    "sessionSlot": "worker",
                    "promptTemplate": "Prompt template for {var}",
                    "action": { "toolName": "fetch", "purpose": "fetch" },
                    "outputVariable": "res"
                }
            ]
        }),
    )
    .await
    .expect("create playbook failed");

    // Update playbook with plain steps array that ONLY changes description and omits promptTemplate and sessionSlot
    let updated = PlaybookService::update_playbook(
        &playbook_repo,
        &session_repo,
        &created.id,
        "sess-service-test",
        None,
        Some(serde_json::json!([
            {
                "stepId": "step_1",
                "description": "Updated Step Description",
                "action": { "toolName": "fetch", "purpose": "fetch" },
                "outputVariable": "res"
            }
        ])),
    )
    .await
    .expect("update playbook failed");

    let pb = Playbook::from_model(&updated);
    assert_eq!(pb.workflow[0].description, "Updated Step Description");
    assert_eq!(pb.workflow[0].session_slot.as_deref(), Some("worker"));
    assert_eq!(
        pb.workflow[0].prompt_template.as_deref(),
        Some("Prompt template for {var}")
    );
    assert!(pb.default_target_session.is_some());
    assert!(pb.session_slots.is_some());
}
