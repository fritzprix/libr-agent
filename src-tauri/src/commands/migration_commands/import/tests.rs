use super::*;

#[cfg(test)]
mod mcp_import_guard_tests {
    use super::*;
    use crate::entity::mcp_server;
    use sea_orm::{ActiveModelTrait, EntityTrait, Set, TransactionTrait};

    async fn txn() -> sea_orm::DatabaseTransaction {
        use sea_orm_migration::MigratorTrait;
        crate::lifecycle::database::register_sqlite_vec();
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("memory db");
        crate::migration::Migrator::up(&db, None)
            .await
            .expect("migrations");
        db.begin().await.expect("transaction")
    }

    fn record(id: &str, config: &str) -> McpServerRecord {
        McpServerRecord {
            id: id.to_string(),
            name: format!("imported-{id}"),
            config: config.to_string(),
            tool_count: None,
            cached_tools: None,
            verification_status: None,
            last_verification_error: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[tokio::test]
    async fn overwrite_import_skips_plugin_owned_rows() {
        let t = txn().await;
        let owned_config = serde_json::json!({
            "name": "from-plugin",
            "transport": { "type": "http", "url": "https://plugin.example/mcp" },
            "pluginSource": "plugin:plugin-ff00000000000001"
        })
        .to_string();
        let active = mcp_server::ActiveModel {
            id: Set("srv-plugin-owned".to_string()),
            name: Set("from-plugin".to_string()),
            config: Set(owned_config.clone()),
            tool_count: Set(None),
            cached_tools: Set(None),
            verification_status: Set(None),
            last_verification_error: Set(None),
            secret_names: Set(None),
            encrypted_secrets: Set(None),
            created_at: Set(0),
            updated_at: Set(0),
        };
        active.insert(&t).await.expect("seed owned row");

        let report = import_mcp_data(
            &t,
            vec![record(
                "srv-plugin-owned",
                r#"{"name":"evil","transport":{"type":"http","url":"https://evil.example"}}"#,
            )],
            "overwrite",
        )
        .await
        .expect("section import");

        assert_eq!(report.success, 0);
        assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
        assert!(
            report.errors[0].contains("owned by plugin"),
            "{:?}",
            report.errors
        );

        let row = mcp_server::Entity::find_by_id("srv-plugin-owned")
            .one(&t)
            .await
            .unwrap()
            .expect("row still exists");
        assert_eq!(row.config, owned_config, "owned config must survive import");
    }

    #[tokio::test]
    async fn overwrite_import_still_updates_normal_rows() {
        let t = txn().await;
        let active = mcp_server::ActiveModel {
            id: Set("srv-normal".to_string()),
            name: Set("old-name".to_string()),
            config: Set(
                r#"{"name":"old","transport":{"type":"http","url":"https://old.example"}}"#
                    .to_string(),
            ),
            tool_count: Set(None),
            cached_tools: Set(None),
            verification_status: Set(None),
            last_verification_error: Set(None),
            secret_names: Set(None),
            encrypted_secrets: Set(None),
            created_at: Set(0),
            updated_at: Set(0),
        };
        active.insert(&t).await.expect("seed normal row");

        let report = import_mcp_data(
            &t,
            vec![record(
                "srv-normal",
                r#"{"name":"new","transport":{"type":"http","url":"https://new.example"}}"#,
            )],
            "overwrite",
        )
        .await
        .expect("section import");

        assert_eq!(report.success, 1, "{:?}", report.errors);
        let row = mcp_server::Entity::find_by_id("srv-normal")
            .one(&t)
            .await
            .unwrap()
            .expect("row still exists");
        assert_eq!(row.name, "imported-srv-normal");
    }
}

// #2923 (D2, gatekeeper G1.1): a migration import must never create,
// overwrite, or wipe plugin-owned assistant rows — those change only with
// their plugin (install/update sync and uninstall).
#[cfg(test)]
mod assistant_import_guard_tests {
    use super::*;
    use crate::entity::assistant;
    use sea_orm::{ActiveModelTrait, EntityTrait, Set, TransactionTrait};

    async fn txn() -> sea_orm::DatabaseTransaction {
        use sea_orm_migration::MigratorTrait;
        crate::lifecycle::database::register_sqlite_vec();
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("memory db");
        crate::migration::Migrator::up(&db, None)
            .await
            .expect("migrations");
        db.begin().await.expect("transaction")
    }

    fn record(id: &str, name: &str, config: &str) -> AssistantRecord {
        AssistantRecord {
            id: id.to_string(),
            name: name.to_string(),
            config: config.to_string(),
            created_at: 0,
            updated_at: 0,
        }
    }

    async fn seed(t: &sea_orm::DatabaseTransaction, id: &str, name: &str, config: &str) {
        let active = assistant::ActiveModel {
            id: Set(id.to_string()),
            name: Set(name.to_string()),
            config: Set(config.to_string()),
            created_at: Set(111),
            updated_at: Set(222),
        };
        active.insert(t).await.expect("seed row");
    }

    fn owned_config() -> String {
        serde_json::json!({
            "systemPrompt": "from-plugin",
            "pluginSource": "plugin:plugin-ff00000000000001"
        })
        .to_string()
    }

    #[tokio::test]
    async fn overwrite_import_leaves_owned_row_byte_identical() {
        let t = txn().await;
        let config = owned_config();
        seed(
            &t,
            "plugin:plugin-ff00000000000001:reviewer",
            "reviewer",
            &config,
        )
        .await;

        let report = import_assistants_data(
            &t,
            vec![record(
                "plugin:plugin-ff00000000000001:reviewer",
                "evil",
                r#"{"systemPrompt":"evil"}"#,
            )],
            "overwrite",
        )
        .await
        .expect("section import");

        assert_eq!(report.success, 0);
        assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
        assert!(
            report.errors[0].contains("read-only"),
            "{:?}",
            report.errors
        );

        let row = assistant::Entity::find_by_id("plugin:plugin-ff00000000000001:reviewer")
            .one(&t)
            .await
            .unwrap()
            .expect("owned row must survive the import");
        assert_eq!(row.config, config, "owned config must be byte-identical");
        assert_eq!(row.name, "reviewer");
        assert_eq!(row.created_at, 111);
        assert_eq!(row.updated_at, 222);
    }

    #[tokio::test]
    async fn import_rejects_records_with_spoofed_marker() {
        let t = txn().await;
        let report = import_assistants_data(
            &t,
            vec![record(
                "user-row",
                "spoofed",
                r#"{"systemPrompt":"x","pluginSource":"plugin:victim"}"#,
            )],
            "skip",
        )
        .await
        .expect("section import");

        assert_eq!(report.success, 0);
        assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
        assert!(
            report.errors[0].contains("pluginSource"),
            "{:?}",
            report.errors
        );
        assert!(assistant::Entity::find_by_id("user-row")
            .one(&t)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn replace_delete_keeps_owned_rows_and_deletes_user_rows() {
        let t = txn().await;
        let config = owned_config();
        seed(
            &t,
            "plugin:plugin-ff00000000000001:reviewer",
            "reviewer",
            &config,
        )
        .await;
        seed(&t, "user-1", "user one", r#"{"systemPrompt":"mine"}"#).await;

        let deleted = delete_non_plugin_assistants(&t)
            .await
            .expect("replace delete");
        assert_eq!(deleted, 1);
        assert!(
            assistant::Entity::find_by_id("plugin:plugin-ff00000000000001:reviewer")
                .one(&t)
                .await
                .unwrap()
                .is_some()
        );
        assert!(assistant::Entity::find_by_id("user-1")
            .one(&t)
            .await
            .unwrap()
            .is_none());
    }
}
