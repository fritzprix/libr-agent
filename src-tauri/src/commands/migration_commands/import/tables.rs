use sea_orm::{ActiveModelTrait, EntityTrait, Set};

use crate::commands::migration_commands::models::{
    MigrationSectionReport, PlaybookRecord, ScheduledTaskRecord, SettingsRecord,
};
use crate::entity::{playbook, scheduled_task, settings};

pub(super) async fn import_settings_data(
    txn: &sea_orm::DatabaseTransaction,
    records: Vec<SettingsRecord>,
    strategy: &str,
) -> Result<MigrationSectionReport, String> {
    let mut success = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();

    for r in records {
        let key = r.key.clone();
        let existing = settings::Entity::find_by_id(&key)
            .one(txn)
            .await
            .map_err(|e| e.to_string())?;

        match (existing, strategy) {
            (Some(_), "skip") => {
                skipped += 1;
            }
            (Some(existing_model), "merge") => {
                let merged_value = if let (Ok(mut old_obj), Ok(new_obj)) = (
                    serde_json::from_str::<serde_json::Value>(&existing_model.value),
                    serde_json::from_str::<serde_json::Value>(&r.value),
                ) {
                    if let (Some(old_map), Some(new_map)) =
                        (old_obj.as_object_mut(), new_obj.as_object())
                    {
                        for (k, v) in new_map {
                            old_map.insert(k.clone(), v.clone());
                        }
                        serde_json::to_string(&old_obj).unwrap_or(r.value)
                    } else {
                        r.value
                    }
                } else {
                    r.value
                };

                let mut active: settings::ActiveModel = existing_model.into();
                active.value = Set(merged_value);
                active.updated_at = Set(chrono::Utc::now().timestamp_millis());
                if let Err(e) = active.update(txn).await {
                    errors.push(format!("Failed to merge setting '{}': {}", key, e));
                } else {
                    success += 1;
                }
            }
            (existing_opt, _) => {
                let now = chrono::Utc::now().timestamp_millis();
                if let Some(existing_model) = existing_opt {
                    let mut active: settings::ActiveModel = existing_model.into();
                    active.value = Set(r.value);
                    active.updated_at = Set(now);
                    if let Err(e) = active.update(txn).await {
                        errors.push(format!("Failed to overwrite setting '{}': {}", key, e));
                    } else {
                        success += 1;
                    }
                } else {
                    let active = settings::ActiveModel {
                        key: Set(r.key),
                        value: Set(r.value),
                        created_at: Set(r.created_at),
                        updated_at: Set(now),
                    };
                    if let Err(e) = settings::Entity::insert(active).exec(txn).await {
                        errors.push(format!("Failed to insert setting '{}': {}", key, e));
                    } else {
                        success += 1;
                    }
                }
            }
        }
    }

    Ok(MigrationSectionReport {
        success,
        skipped,
        errors,
    })
}

/// Delete every assistant row that is NOT plugin-owned (#2923 D2, gatekeeper
/// G1.1). Used by the replace ("overwrite") import instead of a blanket
/// `delete_many`, so a replace import can never wipe the rows a plugin owns;
/// those are removed only by the plugin's uninstall flow.

pub(super) async fn import_playbooks_data(
    txn: &sea_orm::DatabaseTransaction,
    records: Vec<PlaybookRecord>,
    strategy: &str,
) -> Result<MigrationSectionReport, String> {
    let mut success = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();

    for r in records {
        let pk = (r.id.clone(), r.assistant_id.clone());
        let existing = playbook::Entity::find_by_id(pk.clone())
            .one(txn)
            .await
            .map_err(|e| e.to_string())?;

        // For non-settings tables, the "merge" strategy is intentionally treated as "skip" to protect existing user-defined entities.
        if existing.is_some() && (strategy == "skip" || strategy == "merge") {
            skipped += 1;
            continue;
        }

        let now = chrono::Utc::now().timestamp_millis();
        if let Some(existing_model) = existing {
            let mut active: playbook::ActiveModel = existing_model.into();
            active.goal = Set(r.goal);
            active.initial_command = Set(r.initial_command);
            active.workflow = Set(r.workflow);
            active.success_criteria = Set(r.success_criteria);
            active.default_target_session = Set(r.default_target_session);
            active.is_bookmarked = Set(r.is_bookmarked);
            active.updated_at = Set(now);
            if let Err(e) = active.update(txn).await {
                errors.push(format!("Failed to update playbook '{:?}': {}", pk, e));
            } else {
                success += 1;
            }
        } else {
            let active = playbook::ActiveModel {
                id: Set(r.id),
                assistant_id: Set(r.assistant_id),
                goal: Set(r.goal),
                initial_command: Set(r.initial_command),
                workflow: Set(r.workflow),
                success_criteria: Set(r.success_criteria),
                default_target_session: Set(r.default_target_session),
                created_at: Set(r.created_at),
                updated_at: Set(now),
                is_bookmarked: Set(r.is_bookmarked),
            };
            if let Err(e) = playbook::Entity::insert(active).exec(txn).await {
                errors.push(format!("Failed to insert playbook '{:?}': {}", pk, e));
            } else {
                success += 1;
            }
        }
    }

    Ok(MigrationSectionReport {
        success,
        skipped,
        errors,
    })
}

pub(super) async fn import_tasks_data(
    txn: &sea_orm::DatabaseTransaction,
    records: Vec<ScheduledTaskRecord>,
    strategy: &str,
) -> Result<MigrationSectionReport, String> {
    let mut success = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();

    for r in records {
        let id = r.id.clone();
        let existing = scheduled_task::Entity::find_by_id(&id)
            .one(txn)
            .await
            .map_err(|e| e.to_string())?;

        // For non-settings tables, the "merge" strategy is intentionally treated as "skip" to protect existing user-defined entities.
        if existing.is_some() && (strategy == "skip" || strategy == "merge") {
            skipped += 1;
            continue;
        }

        let now = chrono::Utc::now().timestamp_millis();
        let execution_mode = r.resolved_execution_mode();
        if let Some(existing_model) = existing {
            let mut active: scheduled_task::ActiveModel = existing_model.into();
            active.name = Set(r.name);
            active.task_category = Set(r.task_category);
            active.cron_expression = Set(r.cron_expression);
            active.schedule_timezone = Set(r.schedule_timezone);
            active.assistant_id = Set(r.assistant_id);
            active.message = Set(r.message);
            active.execution_mode = Set(execution_mode.clone());
            active.created_by_session_id = Set(r.created_by_session_id);
            active.session_id = Set(r.session_id);
            active.workspace_override = Set(r.workspace_override);
            active.reset_planning_state = Set(r.reset_planning_state);
            active.enabled = Set(r.enabled);
            active.last_run_at = Set(r.last_run_at);
            active.next_run_at = Set(r.next_run_at);
            active.updated_at = Set(now);
            if let Err(e) = active.update(txn).await {
                errors.push(format!("Failed to update scheduled task '{}': {}", id, e));
            } else {
                success += 1;
            }
        } else {
            let active = scheduled_task::ActiveModel {
                id: Set(r.id),
                name: Set(r.name),
                task_category: Set(r.task_category),
                cron_expression: Set(r.cron_expression),
                schedule_timezone: Set(r.schedule_timezone),
                assistant_id: Set(r.assistant_id),
                message: Set(r.message),
                execution_mode: Set(execution_mode),
                created_by_session_id: Set(r.created_by_session_id),
                session_id: Set(r.session_id),
                workspace_override: Set(r.workspace_override),
                reset_planning_state: Set(r.reset_planning_state),
                enabled: Set(r.enabled),
                last_run_at: Set(r.last_run_at),
                next_run_at: Set(r.next_run_at),
                run_claim_run: Set(None),
                run_claim_expires_at: Set(None),
                created_at: Set(r.created_at),
                updated_at: Set(now),
            };
            if let Err(e) = scheduled_task::Entity::insert(active).exec(txn).await {
                errors.push(format!("Failed to insert scheduled task '{}': {}", id, e));
            } else {
                success += 1;
            }
        }
    }

    Ok(MigrationSectionReport {
        success,
        skipped,
        errors,
    })
}

// #2924 (D2, gatekeeper G1.4): a migration import must never overwrite a
// plugin-owned MCP server row — those change only with their plugin.
