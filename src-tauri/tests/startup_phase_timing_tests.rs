//! Warm-start phase timings for database init bottlenecks.
//!
//! Optional: set `LIBRAGENT_STARTUP_BENCH_DB` to a SQLite DB path (or directory
//! containing `libragent_v2.db`) to measure against a realistic copy.
//! Without the env var, the test still measures a freshly migrated temp DB.

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::MigratorTrait;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tauri_mcp_agent_lib::lifecycle::database::init_database;
use tauri_mcp_agent_lib::migration::Migrator;

fn resolve_bench_source_db() -> Option<PathBuf> {
    let raw = std::env::var("LIBRAGENT_STARTUP_BENCH_DB").ok()?;
    let path = PathBuf::from(raw);
    if path.is_file() {
        return Some(path);
    }
    let candidate = path.join("libragent_v2.db");
    if candidate.is_file() {
        return Some(candidate);
    }
    None
}

fn copy_sqlite_tree(src: &Path, dest: &Path) {
    std::fs::copy(src, dest).expect("copy main db");
    for suffix in ["-wal", "-shm"] {
        let src_side = PathBuf::from(format!("{}{}", src.display(), suffix));
        if src_side.is_file() {
            let dest_side = PathBuf::from(format!("{}{}", dest.display(), suffix));
            let _ = std::fs::copy(&src_side, &dest_side);
        }
    }
}

async fn connect_file(path: &Path) -> DatabaseConnection {
    let url = format!("sqlite://{}?mode=rwc", path.display());
    sea_orm::Database::connect(&url)
        .await
        .expect("connect sqlite file")
}

async fn time_vacuum_into(db: &DatabaseConnection, dest: &Path) -> u128 {
    let safe = dest.display().to_string().replace('\'', "''");
    let sql = format!("VACUUM INTO '{}'", safe);
    let start = Instant::now();
    db.execute(Statement::from_string(DbBackend::Sqlite, sql))
        .await
        .expect("VACUUM INTO");
    start.elapsed().as_millis()
}

async fn time_migrator_up(db: &DatabaseConnection) -> u128 {
    let start = Instant::now();
    Migrator::up(db, None).await.expect("Migrator::up");
    start.elapsed().as_millis()
}

#[tokio::test]
async fn measure_startup_db_phases() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let work_db = tmp.path().join("bench.db");

    let source = resolve_bench_source_db();
    let source_label = match &source {
        Some(path) => {
            let meta = std::fs::metadata(path).expect("source metadata");
            copy_sqlite_tree(path, &work_db);
            format!("{} ({} bytes)", path.display(), meta.len())
        }
        None => {
            // Build a fully-migrated DB via init_database so sqlite-vec is registered.
            let seed_url = format!("sqlite://{}?mode=rwc", work_db.display());
            let seeded = init_database(&seed_url)
                .await
                .expect("seed via init_database");
            seeded.close().await.ok();
            "fresh-migrated-temp-db".to_string()
        }
    };

    println!("STARTUP_BENCH source={source_label}");

    let db = connect_file(&work_db).await;

    let vacuum_dest = tmp.path().join("vacuum_copy.db");
    let vacuum_ms = time_vacuum_into(&db, &vacuum_dest).await;
    println!("STARTUP_BENCH phase=vacuum_into ms={vacuum_ms}");

    let migrator_ms = time_migrator_up(&db).await;
    println!("STARTUP_BENCH phase=migrator_up_warm ms={migrator_ms}");

    // Second Migrator::up call should stay cheap if SeaORM skips applied migrations.
    let migrator_ms_2 = time_migrator_up(&db).await;
    println!("STARTUP_BENCH phase=migrator_up_warm_repeat ms={migrator_ms_2}");

    db.close().await.ok();

    // Full init_database path (includes backup + verify + migrator) on a second copy.
    let init_db = tmp.path().join("init_path.db");
    if source.is_some() {
        copy_sqlite_tree(
            &resolve_bench_source_db().expect("source still present"),
            &init_db,
        );
    } else {
        std::fs::copy(&work_db, &init_db).expect("copy seeded db");
    }
    let init_url = format!("sqlite://{}?mode=rwc", init_db.display());
    let init_start = Instant::now();
    let conn = init_database(&init_url).await.expect("init_database");
    let init_ms = init_start.elapsed().as_millis();
    println!("STARTUP_BENCH phase=init_database_total ms={init_ms}");
    conn.close().await.ok();

    // Guardrails: this is a measurement test, not a flaky performance assertion.
    // Keep only sanity checks so CI still passes on tiny DBs.
    assert!(
        migrator_ms_2 <= migrator_ms.saturating_mul(5).saturating_add(2_000),
        "repeat Migrator::up unexpectedly slower: first={migrator_ms}ms second={migrator_ms_2}ms"
    );
}
