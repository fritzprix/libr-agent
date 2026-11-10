use super::key_file::KEY_LEN;
use super::*;
use crate::mcp::secret_templates::{MemorySecretStore, SecretStore};
use crate::repositories::mcp_server_repository::{MCPServerRepository, SqliteMCPServerRepository};
use sea_orm::Database;
use sea_orm_migration::MigratorTrait;

async fn repo() -> SqliteMCPServerRepository {
    crate::lifecycle::database::register_sqlite_vec();
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("memory db");
    crate::migration::Migrator::up(&db, None)
        .await
        .expect("migrations");
    SqliteMCPServerRepository::new(db)
}

async fn server(repo: &SqliteMCPServerRepository) -> String {
    repo.create(
        "exa",
        serde_json::json!({
            "name": "exa",
            "transport": {
                "type": "http",
                "url": "https://mcp.exa.ai/mcp?exaApiKey={{secret:EXA_API_KEY}}"
            }
        }),
    )
    .await
    .expect("create")
    .id
}

#[tokio::test]
async fn without_a_keychain_values_are_encrypted_in_the_row() {
    let repo = repo().await;
    let id = server(&repo).await;
    let dir = tempfile::tempdir().unwrap();
    let keychain = MemorySecretStore::unavailable();
    let store = LayeredSecretStore::new(
        &keychain,
        FileSecretStore::new(&repo, dir.path().join(KEY_FILE_NAME)),
    );

    let notice = store.set(&id, "EXA_API_KEY", "exa-secret").await.unwrap();

    assert_eq!(notice.as_deref(), Some(FALLBACK_WARNING));
    let row = repo.get(&id).await.unwrap().expect("row");
    let column = row.encrypted_secrets.clone().expect("encrypted column");
    assert!(!column.contains("exa-secret"));
    assert!(!row.config.contains("exa-secret"));
    assert!(!format!("{row:?}").contains("exa-secret"));
    assert_eq!(
        store.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
        Some("exa-secret")
    );
    assert_eq!(keychain.get(&id, "EXA_API_KEY").await.unwrap(), None);
}

#[tokio::test]
async fn the_key_file_is_created_once_private_and_reused() {
    let repo = repo().await;
    let id = server(&repo).await;
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join(KEY_FILE_NAME);
    let store = FileSecretStore::new(&repo, key_path.clone());

    let first = store.set(&id, "A_KEY", "one").await.unwrap();
    let key = std::fs::read(&key_path).unwrap();
    let second = store.set(&id, "B_KEY", "two").await.unwrap();
    let column_before = repo.get(&id).await.unwrap().unwrap().encrypted_secrets;
    store.set(&id, "B_KEY", "two").await.unwrap();
    let column_after = repo.get(&id).await.unwrap().unwrap().encrypted_secrets;

    assert_eq!(first.as_deref(), Some(FALLBACK_WARNING));
    assert_eq!(second, None, "the warning shows once");
    assert_eq!(key.len(), KEY_LEN);
    assert_eq!(std::fs::read(&key_path).unwrap(), key, "the key is reused");
    assert_ne!(column_before, column_after, "every write uses a new nonce");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&key_path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name() != KEY_FILE_NAME)
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");

    let reopened = FileSecretStore::new(&repo, key_path);
    assert_eq!(
        reopened.get(&id, "A_KEY").await.unwrap().as_deref(),
        Some("one")
    );
}

#[tokio::test]
async fn deleting_the_last_value_clears_the_column() {
    let repo = repo().await;
    let id = server(&repo).await;
    let dir = tempfile::tempdir().unwrap();
    let store = FileSecretStore::new(&repo, dir.path().join(KEY_FILE_NAME));
    store.set(&id, "A_KEY", "one").await.unwrap();
    store.set(&id, "B_KEY", "two").await.unwrap();

    store.delete(&id, "A_KEY").await.unwrap();
    assert_eq!(store.get(&id, "A_KEY").await.unwrap(), None);
    assert_eq!(
        store.get(&id, "B_KEY").await.unwrap().as_deref(),
        Some("two")
    );

    store.delete(&id, "B_KEY").await.unwrap();
    let row = repo.get(&id).await.unwrap().unwrap();
    assert_eq!(row.encrypted_secrets, None);
}

#[tokio::test]
async fn a_value_map_copied_to_another_row_does_not_decrypt() {
    let repo = repo().await;
    let id = server(&repo).await;
    let other = repo
        .create("other", serde_json::json!({ "name": "other", "transport": { "type": "http", "url": "https://a.example" } }))
        .await
        .unwrap()
        .id;
    let dir = tempfile::tempdir().unwrap();
    let store = FileSecretStore::new(&repo, dir.path().join(KEY_FILE_NAME));
    store.set(&id, "A_KEY", "one").await.unwrap();
    let column = repo.get(&id).await.unwrap().unwrap().encrypted_secrets;
    repo.set_encrypted_secrets(&other, column).await.unwrap();

    let error = store.get(&other, "A_KEY").await.err().expect("fails");
    assert!(error.contains("reenterSecrets"), "{error}");
}

#[tokio::test]
async fn a_working_keychain_is_used_and_the_row_stays_empty() {
    let repo = repo().await;
    let id = server(&repo).await;
    let dir = tempfile::tempdir().unwrap();
    let keychain = MemorySecretStore::default();
    let store = LayeredSecretStore::new(
        &keychain,
        FileSecretStore::new(&repo, dir.path().join(KEY_FILE_NAME)),
    );

    assert_eq!(store.set(&id, "A_KEY", "one").await.unwrap(), None);

    assert_eq!(
        keychain.get(&id, "A_KEY").await.unwrap().as_deref(),
        Some("one")
    );
    assert_eq!(
        repo.get(&id).await.unwrap().unwrap().encrypted_secrets,
        None
    );
    assert!(!dir.path().join(KEY_FILE_NAME).exists());
}

#[tokio::test]
async fn a_keychain_save_removes_the_older_fallback_copy() {
    let repo = repo().await;
    let id = server(&repo).await;
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join(KEY_FILE_NAME);
    let fallback = FileSecretStore::new(&repo, key_path.clone());
    fallback.set(&id, "A_KEY", "old").await.unwrap();
    fallback.set(&id, "B_KEY", "kept").await.unwrap();

    let keychain = MemorySecretStore::default();
    let store = LayeredSecretStore::new(&keychain, FileSecretStore::new(&repo, key_path));
    store.set(&id, "A_KEY", "new").await.unwrap();

    assert_eq!(
        keychain.get(&id, "A_KEY").await.unwrap().as_deref(),
        Some("new")
    );
    assert_eq!(fallback.get(&id, "A_KEY").await.unwrap(), None);
    assert_eq!(
        fallback.get(&id, "B_KEY").await.unwrap().as_deref(),
        Some("kept")
    );
}

#[tokio::test]
async fn a_lost_key_file_still_accepts_a_new_value() {
    let repo = repo().await;
    let id = server(&repo).await;
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join(KEY_FILE_NAME);
    let store = FileSecretStore::new(&repo, key_path.clone());
    store.set(&id, "EXA_API_KEY", "old").await.unwrap();

    std::fs::remove_file(&key_path).unwrap();
    assert!(
        store.get(&id, "EXA_API_KEY").await.is_err(),
        "the old value is gone with the key file"
    );

    // The key file is recreated here, so the pending warning is armed again: the
    // stored values of every row were replaced with unreadable ones.
    assert_eq!(
        store
            .set(&id, "EXA_API_KEY", "new")
            .await
            .unwrap()
            .as_deref(),
        Some(FALLBACK_WARNING)
    );
    assert_eq!(
        store.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
        Some("new")
    );
    assert_eq!(std::fs::read(&key_path).unwrap().len(), KEY_LEN);
}

#[tokio::test]
async fn a_damaged_key_file_is_replaced() {
    let repo = repo().await;
    let id = server(&repo).await;
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join(KEY_FILE_NAME);
    let store = FileSecretStore::new(&repo, key_path.clone());
    store.set(&id, "EXA_API_KEY", "old").await.unwrap();
    std::fs::write(&key_path, b"not-a-key").unwrap();

    assert!(
        store.get(&id, "EXA_API_KEY").await.is_err(),
        "a damaged key cannot read the column"
    );
    store.set(&id, "EXA_API_KEY", "new").await.unwrap();
    assert_eq!(
        store.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
        Some("new")
    );
    assert_eq!(std::fs::read(&key_path).unwrap().len(), KEY_LEN);
}

#[tokio::test]
async fn a_key_file_that_cannot_be_read_still_fails_the_write() {
    let repo = repo().await;
    let id = server(&repo).await;
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join(KEY_FILE_NAME);
    // A directory is never a key, and reading it fails with an error that is not
    // NotFound. That must not be treated as a missing key file.
    std::fs::create_dir(&key_path).unwrap();
    let store = FileSecretStore::new(&repo, key_path);

    let error = store.set(&id, "EXA_API_KEY", "value").await.err();
    assert!(error.is_some(), "a real read error stops the write");
}

#[tokio::test]
async fn the_warning_shows_once_across_stores() {
    let repo = repo().await;
    let id = server(&repo).await;
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join(KEY_FILE_NAME);

    let first = FileSecretStore::new(&repo, key_path.clone());
    assert_eq!(
        first.set(&id, "A_KEY", "one").await.unwrap().as_deref(),
        Some(FALLBACK_WARNING)
    );

    // A restarted app builds a new store on the same key file; the warning was
    // already shown.
    let reopened = FileSecretStore::new(&repo, key_path);
    assert_eq!(reopened.set(&id, "B_KEY", "two").await.unwrap(), None);
}

#[tokio::test]
async fn a_value_rotated_during_an_outage_wins_over_the_stale_keychain_copy() {
    let repo = repo().await;
    let id = server(&repo).await;
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join(KEY_FILE_NAME);
    let keychain = MemorySecretStore::default();
    let store = LayeredSecretStore::new(&keychain, FileSecretStore::new(&repo, key_path.clone()));
    store.set(&id, "EXA_API_KEY", "old").await.unwrap();

    keychain.set_available(false);
    store.set(&id, "EXA_API_KEY", "rotated").await.unwrap();
    assert_eq!(
        keychain.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
        Some("old"),
        "the keychain still holds the stale value"
    );

    // A restart with the keychain back.
    keychain.set_available(true);
    let restarted = LayeredSecretStore::new(&keychain, FileSecretStore::new(&repo, key_path));
    assert_eq!(
        restarted.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
        Some("rotated")
    );
    assert_eq!(
        restarted.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
        Some("rotated")
    );
}

#[tokio::test]
async fn a_fallback_value_is_moved_to_the_keychain_not_copied() {
    let repo = repo().await;
    let id = server(&repo).await;
    let dir = tempfile::tempdir().unwrap();
    let keychain = MemorySecretStore::unavailable();
    let store = LayeredSecretStore::new(
        &keychain,
        FileSecretStore::new(&repo, dir.path().join(KEY_FILE_NAME)),
    );
    store.set(&id, "EXA_API_KEY", "value").await.unwrap();

    keychain.set_available(true);
    assert_eq!(
        store.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
        Some("value")
    );

    assert_eq!(
        keychain.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
        Some("value")
    );
    assert_eq!(
        repo.get(&id).await.unwrap().unwrap().encrypted_secrets,
        None,
        "the fallback copy is cleared"
    );
}

/// A keychain that reports itself available but rejects writes, like a locked one.
struct LockedKeychain;

#[async_trait::async_trait]
impl SecretStore for LockedKeychain {
    async fn get(&self, _: &str, _: &str) -> Result<Option<String>, String> {
        Ok(None)
    }
    async fn set(&self, _: &str, _: &str, _: &str) -> Result<Option<String>, String> {
        Err("the keychain is locked".to_string())
    }
    async fn delete(&self, _: &str, _: &str) -> Result<(), String> {
        Err("the keychain is locked".to_string())
    }
}

#[tokio::test]
async fn a_keychain_that_rejects_the_move_keeps_the_fallback_copy() {
    let repo = repo().await;
    let id = server(&repo).await;
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join(KEY_FILE_NAME);
    FileSecretStore::new(&repo, key_path.clone())
        .set(&id, "EXA_API_KEY", "value")
        .await
        .unwrap();

    let store = LayeredSecretStore::new(&LockedKeychain, FileSecretStore::new(&repo, key_path));
    assert_eq!(
        store.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
        Some("value"),
        "the read still succeeds"
    );
    assert!(repo
        .get(&id)
        .await
        .unwrap()
        .unwrap()
        .encrypted_secrets
        .is_some());
}

#[tokio::test]
async fn without_a_keychain_deleting_the_last_value_clears_the_column_and_creates_no_key() {
    let repo = repo().await;
    let id = server(&repo).await;
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join(KEY_FILE_NAME);
    let keychain = MemorySecretStore::unavailable();
    let store = LayeredSecretStore::new(&keychain, FileSecretStore::new(&repo, key_path.clone()));

    store.delete(&id, "EXA_API_KEY").await.unwrap();
    assert!(!key_path.exists(), "a delete does not create the key file");

    store.set(&id, "EXA_API_KEY", "value").await.unwrap();
    store.delete(&id, "EXA_API_KEY").await.unwrap();
    assert_eq!(
        repo.get(&id).await.unwrap().unwrap().encrypted_secrets,
        None
    );
}

#[tokio::test]
async fn a_delete_during_an_outage_reaches_the_keychain_without_a_read_of_that_name() {
    let repo = repo().await;
    let id = server(&repo).await;
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join(KEY_FILE_NAME);
    let keychain = MemorySecretStore::default();
    let store = LayeredSecretStore::new(&keychain, FileSecretStore::new(&repo, key_path.clone()));
    store.set(&id, "EXA_API_KEY", "value").await.unwrap();

    keychain.set_available(false);
    store.delete(&id, "EXA_API_KEY").await.unwrap();
    assert_eq!(store.get(&id, "EXA_API_KEY").await.unwrap(), None);
    // The row goes away with the server; the pending delete does not.
    repo.delete(&id).await.unwrap();

    // A restart with the keychain back; any operation applies the pending delete.
    keychain.set_available(true);
    let restarted = LayeredSecretStore::new(&keychain, FileSecretStore::new(&repo, key_path));
    assert_eq!(restarted.get("other", "OTHER_KEY").await.unwrap(), None);
    assert_eq!(keychain.get(&id, "EXA_API_KEY").await.unwrap(), None);
    assert!(!dir.path().join(PENDING_DELETES_FILE_NAME).exists());
}

#[tokio::test]
async fn a_value_saved_after_an_outage_delete_is_not_removed_by_it() {
    let repo = repo().await;
    let id = server(&repo).await;
    let dir = tempfile::tempdir().unwrap();
    let keychain = MemorySecretStore::default();
    let store = LayeredSecretStore::new(
        &keychain,
        FileSecretStore::new(&repo, dir.path().join(KEY_FILE_NAME)),
    );
    store.set(&id, "EXA_API_KEY", "old").await.unwrap();

    keychain.set_available(false);
    store.delete(&id, "EXA_API_KEY").await.unwrap();
    store.set(&id, "EXA_API_KEY", "new").await.unwrap();

    keychain.set_available(true);
    assert_eq!(
        store.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
        Some("new")
    );
    store.set(&id, "B_KEY", "other").await.unwrap();
    assert_eq!(
        keychain.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
        Some("new")
    );
}

/// A keychain that saves values but cannot delete them.
#[derive(Default)]
struct UndeletableKeychain(MemorySecretStore);

#[async_trait::async_trait]
impl SecretStore for UndeletableKeychain {
    async fn get(&self, server_id: &str, name: &str) -> Result<Option<String>, String> {
        self.0.get(server_id, name).await
    }
    async fn set(
        &self,
        server_id: &str,
        name: &str,
        value: &str,
    ) -> Result<Option<String>, String> {
        self.0.set(server_id, name, value).await
    }
    async fn delete(&self, _: &str, _: &str) -> Result<(), String> {
        Err("the keychain rejected the delete".to_string())
    }
}

#[tokio::test]
async fn a_failed_pending_delete_does_not_remove_a_value_saved_later() {
    let repo = repo().await;
    let id = server(&repo).await;
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join(KEY_FILE_NAME);
    LayeredSecretStore::new(
        &MemorySecretStore::unavailable(),
        FileSecretStore::new(&repo, key_path.clone()),
    )
    .delete(&id, "EXA_API_KEY")
    .await
    .unwrap();

    let keychain = UndeletableKeychain::default();
    let store = LayeredSecretStore::new(&keychain, FileSecretStore::new(&repo, key_path));
    store.set(&id, "EXA_API_KEY", "new").await.unwrap();

    assert!(
        !dir.path().join(PENDING_DELETES_FILE_NAME).exists(),
        "the save drops the pending delete of the same name"
    );
    assert_eq!(
        keychain.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
        Some("new")
    );
}

#[tokio::test]
async fn an_unreadable_pending_list_still_clears_the_fallback_copy() {
    let repo = repo().await;
    let id = server(&repo).await;
    let dir = tempfile::tempdir().unwrap();
    let keychain = MemorySecretStore::unavailable();
    let store = LayeredSecretStore::new(
        &keychain,
        FileSecretStore::new(&repo, dir.path().join(KEY_FILE_NAME)),
    );
    store.set(&id, "EXA_API_KEY", "value").await.unwrap();
    std::fs::write(dir.path().join(PENDING_DELETES_FILE_NAME), b"not json").unwrap();

    assert!(store.delete(&id, "EXA_API_KEY").await.is_err());
    assert_eq!(
        repo.get(&id).await.unwrap().unwrap().encrypted_secrets,
        None
    );
}

#[tokio::test]
async fn a_pending_delete_that_fails_stays_for_the_next_try() {
    let repo = repo().await;
    let id = server(&repo).await;
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join(KEY_FILE_NAME);
    let keychain = MemorySecretStore::unavailable();
    LayeredSecretStore::new(&keychain, FileSecretStore::new(&repo, key_path.clone()))
        .delete(&id, "EXA_API_KEY")
        .await
        .unwrap();

    let locked = LayeredSecretStore::new(&LockedKeychain, FileSecretStore::new(&repo, key_path));
    assert_eq!(locked.get(&id, "B_KEY").await.unwrap(), None);
    assert!(dir.path().join(PENDING_DELETES_FILE_NAME).exists());
}

#[tokio::test]
async fn deleting_the_only_secret_server_during_an_outage_sweeps_keychain_on_startup() {
    let repo = repo().await;
    let id = server(&repo).await;
    repo.set_secret_names(&id, Some(r#"["EXA_API_KEY"]"#.to_string()))
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join(KEY_FILE_NAME);
    let keychain = MemorySecretStore::default();
    let store = LayeredSecretStore::new(&keychain, FileSecretStore::new(&repo, key_path.clone()));
    store.set(&id, "EXA_API_KEY", "secret-val").await.unwrap();
    assert_eq!(
        keychain.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
        Some("secret-val")
    );

    keychain.set_available(false);
    crate::services::McpServerService::delete_server_config_with_store(&repo, &store, &id)
        .await
        .expect("delete server during outage");

    assert_eq!(repo.get(&id).await.unwrap(), None);
    assert!(dir.path().join(PENDING_DELETES_FILE_NAME).exists());

    keychain.set_available(true);
    assert_eq!(
        keychain.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
        Some("secret-val")
    );

    let restarted = LayeredSecretStore::new(&keychain, FileSecretStore::new(&repo, key_path));
    restarted.sweep_pending_deletes_at_startup().await;

    assert_eq!(keychain.get(&id, "EXA_API_KEY").await.unwrap(), None);
    assert!(!dir.path().join(PENDING_DELETES_FILE_NAME).exists());
}

#[tokio::test]
async fn startup_sweep_does_not_touch_keychain_when_no_pending_file() {
    let repo = repo().await;
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join(KEY_FILE_NAME);

    struct PanickingStore;
    #[async_trait::async_trait]
    impl SecretStore for PanickingStore {
        async fn get(&self, _: &str, _: &str) -> Result<Option<String>, String> {
            panic!("should not be called");
        }
        async fn set(&self, _: &str, _: &str, _: &str) -> Result<Option<String>, String> {
            panic!("should not be called");
        }
        async fn delete(&self, _: &str, _: &str) -> Result<(), String> {
            panic!("should not be called");
        }
        fn available(&self) -> bool {
            panic!("available() should not be called when pending deletes file does not exist");
        }
    }

    let store = LayeredSecretStore::new(&PanickingStore, FileSecretStore::new(&repo, key_path));
    store.sweep_pending_deletes_at_startup().await;
}

#[tokio::test]
async fn startup_sweep_keeps_entries_when_keychain_still_unavailable() {
    let repo = repo().await;
    let id = server(&repo).await;
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join(KEY_FILE_NAME);
    let keychain = MemorySecretStore::unavailable();
    let store = LayeredSecretStore::new(&keychain, FileSecretStore::new(&repo, key_path.clone()));

    store.delete(&id, "EXA_API_KEY").await.unwrap();
    assert!(dir.path().join(PENDING_DELETES_FILE_NAME).exists());

    store.sweep_pending_deletes_at_startup().await;

    assert!(dir.path().join(PENDING_DELETES_FILE_NAME).exists());
}
