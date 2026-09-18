use crate::helpers::column_exists;
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Statement;
use serde_json::Value;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !column_exists(manager, "playbooks", "default_target_session").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Playbooks::Table)
                        .add_column(
                            ColumnDef::new(Playbooks::DefaultTargetSession)
                                .text()
                                .null(),
                        )
                        .to_owned(),
                )
                .await?;
        }

        migrate_envelope_workflow_to_column(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if column_exists(manager, "playbooks", "default_target_session").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Playbooks::Table)
                        .drop_column(Playbooks::DefaultTargetSession)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

/// Split legacy `{ steps, defaultTargetSession }` envelopes into steps-only `workflow`
/// plus a dedicated `default_target_session` column. Array workflows stay unchanged.
async fn migrate_envelope_workflow_to_column(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let db = manager.get_connection();
    let backend = manager.get_database_backend();

    let rows = db
        .query_all(Statement::from_string(
            backend,
            "SELECT id, assistant_id, workflow FROM playbooks".to_owned(),
        ))
        .await?;

    for row in rows {
        let id: String = row.try_get("", "id")?;
        let assistant_id: String = row.try_get("", "assistant_id")?;
        let workflow_str: String = row.try_get("", "workflow")?;

        let Ok(value) = serde_json::from_str::<Value>(&workflow_str) else {
            continue;
        };

        let Value::Object(map) = value else {
            // Already a steps array (or unexpected scalar) — leave as-is.
            continue;
        };

        let Some(steps) = map.get("steps") else {
            continue;
        };

        let pin_json = map.get("defaultTargetSession").and_then(valid_pin_json);

        let steps_json = serde_json::to_string(steps)
            .map_err(|e| DbErr::Custom(format!("Failed to serialize steps: {e}")))?;

        db.execute(Statement::from_sql_and_values(
            backend,
            "UPDATE playbooks SET workflow = ?, default_target_session = ? WHERE id = ? AND assistant_id = ?",
            [
                steps_json.into(),
                pin_json.into(),
                id.into(),
                assistant_id.into(),
            ],
        ))
        .await?;
    }

    Ok(())
}

/// Persist only valid pin/self configs; drop `mode: pin` without sessionId.
fn valid_pin_json(value: &Value) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let obj = value.as_object()?;
    let mode = obj.get("mode")?.as_str()?;
    if mode == "pin" {
        let session_id = obj
            .get("sessionId")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())?;
        return serde_json::to_string(&serde_json::json!({
            "mode": "pin",
            "sessionId": session_id,
        }))
        .ok();
    }
    if mode == "self" {
        return serde_json::to_string(value).ok();
    }
    None
}

#[derive(DeriveIden)]
enum Playbooks {
    #[sea_orm(iden = "playbooks")]
    Table,
    DefaultTargetSession,
}
