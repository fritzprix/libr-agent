use super::*;

#[tokio::test]
async fn user_prompt_submit_returns_additional_context_and_block() {
    let json_out = r#"{"additionalContext":"turn-only extra context"}"#;
    let ps = format!("Write-Output '{json_out}'");
    let sh = format!("printf '%s' '{json_out}'");
    let hooks_json = serde_json::json!({
        "hooks": {
            "UserPromptSubmit": [{
                "hooks": [{
                    "type": "command",
                    "command": sh,
                    "powershell": ps,
                }]
            }]
        }
    })
    .to_string();
    let (temp, root) = setup_approved_plugin(&hooks_json);

    let sid = format!("prompt-submit-{}", uuid::Uuid::new_v4());
    let mut params = HookEnvelopeParams::new(&sid, root, HookEvent::UserPromptSubmit);
    params.prompt = Some("hello".to_string());
    let outcome =
        dispatch_hook_event_in(temp.path(), HookEvent::UserPromptSubmit, "", &params, &sid).await;
    assert_eq!(
        outcome.joined_additional_context().as_deref(),
        Some("turn-only extra context")
    );
    assert_eq!(outcome.decision, None);
    // Must NOT pollute persistent SessionStart context store.
    assert!(get_hook_context(&sid).is_none());
    clear_hook_context(&sid);
}

#[tokio::test]
async fn post_tool_use_and_stop_and_session_end_and_notification_and_pre_compact_execute() {
    let marker_dir = tempfile::tempdir().expect("marker dir");
    let end_marker = marker_dir.path().join("session-end.txt");
    let notif_marker = marker_dir.path().join("notif.txt");
    let compact_marker = marker_dir.path().join("compact.txt");

    let end_str = end_marker.to_string_lossy().to_string();
    let notif_str = notif_marker.to_string_lossy().to_string();
    let compact_str = compact_marker.to_string_lossy().to_string();

    let stop_json = r#"{"decision":"block","reason":"verify tests before stopping"}"#;
    let post_json = r#"{"additionalContext":"post-tool hint"}"#;

    let hooks_json = serde_json::json!({
        "hooks": {
            "Stop": [{
                "command": format!("printf '%s' '{stop_json}'"),
                "powershell": format!("Write-Output '{stop_json}'"),
            }],
            "PostToolUse": [{
                "matcher": "workspace__writeFile",
                "hooks": [{
                    "type": "command",
                    "command": format!("printf '%s' '{post_json}'"),
                    "powershell": format!("Write-Output '{post_json}'"),
                }]
            }],
            "SessionEnd": [{
                "command": format!("sh -c 'echo end > \"{end_str}\"'"),
                "powershell": format!("Set-Content -Path '{end_str}' -Value 'end'"),
            }],
            "Notification": [{
                "matcher": "permission",
                "hooks": [{
                    "type": "command",
                    "command": format!("sh -c 'echo notif > \"{notif_str}\"'"),
                    "powershell": format!("Set-Content -Path '{notif_str}' -Value 'notif'"),
                }]
            }],
            "PreCompact": [{
                "matcher": "auto",
                "hooks": [{
                    "type": "command",
                    "command": format!("sh -c 'echo compact > \"{compact_str}\"'"),
                    "powershell": format!("Set-Content -Path '{compact_str}' -Value 'compact'"),
                }]
            }]
        }
    })
    .to_string();
    let (temp, root) = setup_approved_plugin(&hooks_json);
    let sid = format!("multi-event-{}", uuid::Uuid::new_v4());

    let stop_res = dispatch_hook_event_in(
        temp.path(),
        HookEvent::Stop,
        "",
        &HookEnvelopeParams::new(&sid, root.clone(), HookEvent::Stop),
        &sid,
    )
    .await;
    assert_eq!(stop_res.decision, Some(HookDecision::Block));
    assert_eq!(
        stop_res.reason.as_deref(),
        Some("verify tests before stopping")
    );

    let post_res = dispatch_hook_event_in(
        temp.path(),
        HookEvent::PostToolUse,
        "workspace__writeFile",
        &HookEnvelopeParams::new(&sid, root.clone(), HookEvent::PostToolUse),
        &sid,
    )
    .await;
    assert_eq!(
        post_res.joined_additional_context().as_deref(),
        Some("post-tool hint")
    );

    let _ = dispatch_hook_event_in(
        temp.path(),
        HookEvent::SessionEnd,
        "delete",
        &HookEnvelopeParams::new(&sid, root.clone(), HookEvent::SessionEnd),
        &sid,
    )
    .await;
    assert!(end_marker.is_file(), "SessionEnd handler should execute");

    let _ = dispatch_hook_event_in(
        temp.path(),
        HookEvent::Notification,
        "permission",
        &HookEnvelopeParams::new(&sid, root.clone(), HookEvent::Notification),
        &sid,
    )
    .await;
    assert!(
        notif_marker.is_file(),
        "Notification handler should execute"
    );

    let _ = dispatch_hook_event_in(
        temp.path(),
        HookEvent::PreCompact,
        "auto",
        &HookEnvelopeParams::new(&sid, root, HookEvent::PreCompact),
        &sid,
    )
    .await;
    assert!(
        compact_marker.is_file(),
        "PreCompact handler should execute"
    );
    clear_hook_context(&sid);
}

#[test]
fn decision_aggregation_precedence_is_block_over_ask_over_allow() {
    let mut res = HookDispatchResult::default();
    res.merge(HookOutput {
        decision: Some(HookDecision::Allow),
        reason: Some("allowed".to_string()),
        ..HookOutput::default()
    });
    assert_eq!(res.decision, Some(HookDecision::Allow));

    res.merge(HookOutput {
        decision: Some(HookDecision::Ask),
        reason: Some("confirm".to_string()),
        ..HookOutput::default()
    });
    assert_eq!(res.decision, Some(HookDecision::Ask));
    assert_eq!(res.reason.as_deref(), Some("confirm"));

    res.merge(HookOutput {
        decision: Some(HookDecision::Block),
        reason: Some("denied".to_string()),
        ..HookOutput::default()
    });
    assert_eq!(res.decision, Some(HookDecision::Block));
    assert_eq!(res.reason.as_deref(), Some("denied"));

    // Subsequent Allow/Ask cannot downgrade Block.
    res.merge(HookOutput {
        decision: Some(HookDecision::Allow),
        reason: Some("too late".to_string()),
        ..HookOutput::default()
    });
    assert_eq!(res.decision, Some(HookDecision::Block));
    assert_eq!(res.reason.as_deref(), Some("denied"));
}
// --- #2825 session-scoped plugin sources ---------------------------------
