// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // SQLite 데이터베이스 경로 설정 - 사용자 데이터 디렉토리에 저장
    let db_path = std::env::var("SYNAPTICFLOW_DB_PATH").unwrap_or_else(|_| {
        // 기본적으로 사용자 데이터 디렉토리에 저장
        let data_dir = dirs::data_dir()
            .expect("Failed to get data directory")
            .join("com.fritzprix.synapticflow");
        // Use a different filename to avoid potential locking issues
        data_dir
            .join("synapticflow_v2.db")
            .to_string_lossy()
            .to_string()
    });

    // 데이터베이스 디렉토리가 존재하는지 확인하고 생성
    if let Some(parent_dir) = std::path::Path::new(&db_path).parent() {
        std::fs::create_dir_all(parent_dir).expect("Failed to create database directory");
    }

    let db_url = format!("sqlite://{db_path}");

    println!("🚀 Starting SynapticFlow with SQLite database: {db_url}");

    tauri_mcp_agent_lib::run_with_sqlite_sync(db_url)
}
