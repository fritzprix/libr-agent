use log::error;
use std::sync::OnceLock;
use tauri::Manager;
use tauri_plugin_log::{Target, TargetKind};

mod commands;
mod mcp;
mod services;

use commands::browser_commands::*;
use mcp::{MCPResponse, MCPServerConfig, MCPServerManager};
use services::{InteractiveBrowserServer, SecureFileManager};

// 전역 MCP 서버 매니저
static MCP_MANAGER: OnceLock<MCPServerManager> = OnceLock::new();

fn get_mcp_manager() -> &'static MCPServerManager {
    MCP_MANAGER.get_or_init(MCPServerManager::new)
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {name}! You've been greeted from Rust!")
}

#[tauri::command]
async fn start_mcp_server(config: MCPServerConfig) -> Result<String, String> {
    get_mcp_manager()
        .start_server(config)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn stop_mcp_server(server_name: String) -> Result<(), String> {
    get_mcp_manager()
        .stop_server(&server_name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn call_mcp_tool(
    server_name: String,
    tool_name: String,
    arguments: serde_json::Value,
) -> MCPResponse {
    get_mcp_manager()
        .call_tool(&server_name, &tool_name, arguments)
        .await
}

#[tauri::command]
async fn sample_from_mcp_server(
    server_name: String,
    prompt: String,
    options: Option<serde_json::Value>,
) -> Result<MCPResponse, String> {
    let sampling_options = if let Some(opts) = options {
        Some(
            serde_json::from_value::<mcp::SamplingOptions>(opts)
                .map_err(|e| format!("Invalid sampling options: {}", e))?,
        )
    } else {
        None
    };

    let request = mcp::SamplingRequest {
        prompt,
        options: sampling_options,
    };

    Ok(get_mcp_manager()
        .sample_from_model(&server_name, request)
        .await)
}

#[tauri::command]
async fn list_mcp_tools(server_name: String) -> Result<Vec<mcp::MCPTool>, String> {
    get_mcp_manager()
        .list_tools(&server_name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_tools_from_config(config: serde_json::Value) -> Result<Vec<mcp::MCPTool>, String> {
    println!("🚀 [TAURI] list_tools_from_config called!");
    println!(
        "🚀 [TAURI] Config received: {}",
        serde_json::to_string_pretty(&config).unwrap_or_default()
    );

    // Claude format을 지원: mcpServers 또는 servers 배열을 처리
    let servers_config =
        if let Some(mcp_servers) = config.get("mcpServers").and_then(|v| v.as_object()) {
            // Claude format: mcpServers 객체를 MCPServerConfig 배열로 변환
            println!("🚀 [TAURI] Processing Claude format (mcpServers)");
            let mut server_list = Vec::new();

            for (name, server_config) in mcp_servers.iter() {
                let mut server_value = server_config.clone();
                // name 필드 추가
                if let serde_json::Value::Object(ref mut obj) = server_value {
                    obj.insert("name".to_string(), serde_json::Value::String(name.clone()));
                    obj.insert(
                        "transport".to_string(),
                        serde_json::Value::String("stdio".to_string()),
                    );
                }
                let server_cfg: mcp::MCPServerConfig = serde_json::from_value(server_value)
                    .map_err(|e| format!("Invalid server config: {e}"))?;
                server_list.push(server_cfg);
            }
            server_list
        } else if let Some(servers_array) = config.get("servers").and_then(|v| v.as_array()) {
            // 기존 format: servers 배열
            println!("🚀 [TAURI] Processing legacy format (servers array)");
            let mut server_list = Vec::new();
            for server_value in servers_array {
                let server_cfg: mcp::MCPServerConfig = serde_json::from_value(server_value.clone())
                    .map_err(|e| format!("Invalid server config: {e}"))?;
                server_list.push(server_cfg);
            }
            server_list
        } else {
            return Err("Invalid config: missing mcpServers object or servers array".to_string());
        };

    println!(
        "🚀 [TAURI] Found {} servers in config",
        servers_config.len()
    );

    let manager = get_mcp_manager();

    let mut all_tools: Vec<mcp::MCPTool> = Vec::new();

    // Start servers from config and collect their tools
    for server_cfg in servers_config {
        let server_name = server_cfg.name.clone();
        if !manager.is_server_alive(&server_name).await {
            println!("🚀 [TAURI] Starting server: {server_name}");
            if let Err(e) = manager.start_server(server_cfg).await {
                eprintln!("❌ [TAURI] Failed to start server {server_name}: {e}");
                continue; // Skip to the next server if this one fails to start
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        } else {
            println!("🚀 [TAURI] Server {server_name} already running");
        }

        // Fetch tools for the server we just ensured is running
        match manager.list_tools(&server_name).await {
            Ok(mut tools) => {
                println!(
                    "✅ [TAURI] Found {} tools for server '{}'",
                    tools.len(),
                    server_name
                );
                // Prefix tool names with server name to avoid conflicts
                for tool in &mut tools {
                    tool.name = format!("{}__{}", server_name, tool.name);
                }
                all_tools.extend(tools);
            }
            Err(e) => {
                eprintln!("❌ [TAURI] Error listing tools for '{server_name}': {e}");
                // Continue to the next server
            }
        }
    }

    println!("✅ [TAURI] Total tools collected: {}", all_tools.len());
    Ok(all_tools)
}

#[tauri::command]
async fn get_connected_servers() -> Vec<String> {
    get_mcp_manager().get_connected_servers().await
}

#[tauri::command]
async fn check_server_status(server_name: String) -> bool {
    get_mcp_manager().is_server_alive(&server_name).await
}

#[tauri::command]
async fn check_all_servers_status() -> std::collections::HashMap<String, bool> {
    get_mcp_manager().check_all_servers().await
}

#[tauri::command]
async fn list_all_tools() -> Result<Vec<mcp::MCPTool>, String> {
    get_mcp_manager()
        .list_all_tools()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_validated_tools(server_name: String) -> Result<Vec<mcp::MCPTool>, String> {
    get_mcp_manager()
        .get_validated_tools(&server_name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn validate_tool_schema(tool: mcp::MCPTool) -> Result<(), String> {
    mcp::MCPServerManager::validate_tool_schema(&tool).map_err(|e| e.to_string())
}

// Built-in MCP server commands

#[tauri::command]
async fn list_builtin_servers() -> Vec<String> {
    get_mcp_manager().list_builtin_servers().await
}

#[tauri::command]
async fn list_builtin_tools(server_name: Option<String>) -> Vec<mcp::MCPTool> {
    match server_name {
        Some(name) => get_mcp_manager().list_builtin_tools_for(&name).await,
        None => get_mcp_manager().list_builtin_tools().await,
    }
}

#[tauri::command]
async fn call_builtin_tool(
    server_name: String,
    tool_name: String,
    arguments: serde_json::Value,
) -> mcp::MCPResponse {
    get_mcp_manager()
        .call_builtin_tool(&server_name, &tool_name, arguments)
        .await
}

// 로그 파일 관리 명령들
#[tauri::command]
async fn get_app_logs_dir() -> Result<String, String> {
    use std::env;
    use std::path::PathBuf;

    // 플랫폼에 따른 로그 디렉토리 경로 결정 (Tauri 공식 경로)
    #[cfg(target_os = "windows")]
    let log_dir = {
        let local_appdata = env::var("LOCALAPPDATA")
            .map_err(|_| "Failed to get LOCALAPPDATA directory".to_string())?;
        PathBuf::from(local_appdata)
            .join("com.synaptic-flow.app")
            .join("logs")
    };

    #[cfg(target_os = "macos")]
    let log_dir = {
        let home = env::var("HOME").map_err(|_| "Failed to get HOME directory".to_string())?;
        PathBuf::from(home)
            .join("Library")
            .join("Logs")
            .join("com.synaptic-flow.app")
    };

    #[cfg(target_os = "linux")]
    let log_dir = {
        let home = env::var("HOME").map_err(|_| "Failed to get HOME directory".to_string())?;
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("com.synaptic-flow.app")
            .join("logs")
    };

    Ok(log_dir.to_string_lossy().to_string())
}

#[tauri::command]
async fn backup_current_log() -> Result<String, String> {
    use chrono::Utc;
    use std::fs;

    let log_dir_str = get_app_logs_dir().await?;
    let log_dir = std::path::PathBuf::from(log_dir_str);

    // 현재 로그 파일 찾기 (명시된 파일명 사용)
    let log_file = log_dir.join("synaptic-flow.log");

    if !log_file.exists() {
        return Err("No current log file found".to_string());
    }

    // 백업 파일명 생성 (타임스탬프 포함)
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_file = log_dir.join(format!("synaptic-flow_{}.log.bak", timestamp));

    // 파일 복사
    fs::copy(&log_file, &backup_file).map_err(|e| format!("Failed to backup log file: {}", e))?;

    Ok(backup_file.to_string_lossy().to_string())
}

#[tauri::command]
async fn clear_current_log() -> Result<(), String> {
    use std::fs;

    let log_dir_str = get_app_logs_dir().await?;
    let log_dir = std::path::PathBuf::from(log_dir_str);
    let log_file = log_dir.join("synaptic-flow.log");

    if log_file.exists() {
        fs::write(&log_file, "").map_err(|e| format!("Failed to clear log file: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
async fn list_log_files() -> Result<Vec<String>, String> {
    use std::fs;

    let log_dir_str = get_app_logs_dir().await?;
    let log_dir = std::path::PathBuf::from(log_dir_str);

    if !log_dir.exists() {
        return Ok(vec![]);
    }

    let entries =
        fs::read_dir(&log_dir).map_err(|e| format!("Failed to read log directory: {}", e))?;

    let mut log_files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        if path.is_file() {
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.ends_with(".log") || filename.ends_with(".log.bak") {
                    log_files.push(filename.to_string());
                }
            }
        }
    }

    log_files.sort();
    Ok(log_files)
}

#[tauri::command]
async fn read_file(
    file_path: String,
    manager: tauri::State<'_, SecureFileManager>,
) -> Result<Vec<u8>, String> {
    manager.read_file(&file_path).await
}

#[tauri::command]
async fn read_dropped_file(file_path: String) -> Result<Vec<u8>, String> {
    use std::path::Path;
    use tokio::fs;

    let path = Path::new(&file_path);

    // Basic security checks for dropped files
    if !path.exists() {
        return Err(format!("File does not exist: {}", file_path));
    }

    if !path.is_file() {
        return Err(format!("Path is not a file: {}", file_path));
    }

    // Check file size (10MB limit)
    if let Ok(metadata) = fs::metadata(path).await {
        const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10MB
        if metadata.len() > MAX_FILE_SIZE {
            return Err(format!(
                "File too large: {} bytes (max: {} bytes)",
                metadata.len(),
                MAX_FILE_SIZE
            ));
        }
    }

    // Only allow specific file extensions
    let allowed_extensions = ["txt", "md", "json", "pdf", "docx", "xlsx"];
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase());

    match extension {
        Some(ext) if allowed_extensions.contains(&ext.as_str()) => {
            // Extension is allowed, proceed with reading
        }
        _ => {
            return Err(format!(
                "File type not allowed. Supported: {}",
                allowed_extensions.join(", ")
            ));
        }
    }

    // Read the file
    fs::read(path)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))
}

#[tauri::command]
async fn write_file(
    file_path: String,
    content: Vec<u8>,
    manager: tauri::State<'_, SecureFileManager>,
) -> Result<(), String> {
    manager.write_file(&file_path, &content).await
}

#[tauri::command]
async fn open_external_url(url: String) -> Result<(), String> {
    // URL validation
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Only HTTP/HTTPS URLs are allowed".to_string());
    }

    // Use tauri-plugin-opener to open URL in external browser
    tauri_plugin_opener::open_url(&url, None::<&str>)
        .map_err(|e| format!("Failed to open URL: {}", e))?;

    Ok(())
}

#[tauri::command]
async fn list_all_tools_unified() -> Result<Vec<mcp::MCPTool>, String> {
    get_mcp_manager()
        .list_all_tools_unified()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn call_tool_unified(
    server_name: String,
    tool_name: String,
    arguments: serde_json::Value,
) -> mcp::MCPResponse {
    get_mcp_manager()
        .call_tool_unified(&server_name, &tool_name, arguments)
        .await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Set up custom panic handler for better error reporting
    std::panic::set_hook(Box::new(|panic_info| {
        error!("🚨 PANIC: {panic_info}");
        if let Some(location) = panic_info.location() {
            error!(
                "  Location: {}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            );
        }

        // Attempt graceful shutdown
        error!("🔄 Attempting graceful shutdown...");
    }));

    // Configure Tauri builder with error handling
    let result = std::panic::catch_unwind(|| {
        tauri::Builder::default()
            .plugin(tauri_plugin_dialog::init())
            .plugin(
                tauri_plugin_log::Builder::new()
                    .targets([
                        Target::new(TargetKind::Stdout),
                        Target::new(TargetKind::LogDir {
                            file_name: Some("synaptic-flow".to_string()),
                        }),
                        Target::new(TargetKind::Webview),
                    ])
                    .level(log::LevelFilter::Info)
                    .build(),
            )
            .plugin(tauri_plugin_opener::init())
            .invoke_handler(tauri::generate_handler![
                greet,
                start_mcp_server,
                stop_mcp_server,
                call_mcp_tool,
                sample_from_mcp_server,
                list_mcp_tools,
                list_tools_from_config,
                get_connected_servers,
                check_server_status,
                check_all_servers_status,
                list_all_tools,
                get_validated_tools,
                validate_tool_schema,
                list_builtin_servers,
                list_builtin_tools,
                call_builtin_tool,
                list_all_tools_unified,
                call_tool_unified,
                get_app_logs_dir,
                backup_current_log,
                clear_current_log,
                list_log_files,
                read_file,
                read_dropped_file,
                write_file,
                open_external_url,
                // Interactive Browser commands
                create_browser_session,
                close_browser_session,
                click_element,
                input_text,
                scroll_page,
                get_current_url,
                get_page_title,
                element_exists,
                list_browser_sessions,
                navigate_to_url,
                get_page_content,
                take_screenshot,
                browser_script_result,
                execute_script,
                poll_script_result,
                navigate_back,
                navigate_forward,
                get_element_text,
                get_element_attribute,
                find_element
            ])
            .setup(|app| {
                println!("🚀 SynapticFlow initializing...");

                // Initialize SecureFileManager
                let file_manager = SecureFileManager::new();
                app.manage(file_manager);
                println!("✅ SecureFileManager initialized");

                // Initialize Interactive Browser Server
                let browser_server = InteractiveBrowserServer::new(app.handle().clone());
                app.manage(browser_server);
                println!("✅ Interactive Browser Server initialized");

                // Initialize builtin servers with AppHandle for Browser Agent support
                tauri::async_runtime::spawn(async move {
                    get_mcp_manager().initialize_builtin_servers().await;
                    println!("✅ Builtin servers initialized with Browser Agent support");
                });

                // Verify WebView can be created safely
                #[cfg(target_os = "linux")]
                {
                    println!("🐧 Linux detected - checking WebKit compatibility...");

                    // Set environment variables for better WebKit compatibility
                    std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
                    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

                    // Check if running in a container or limited environment
                    if std::env::var("container").is_ok() || std::env::var("DISPLAY").is_err() {
                        eprintln!("⚠️  Warning: Running in limited graphics environment");
                    }
                }

                println!("✅ SynapticFlow setup completed successfully");
                Ok(())
            })
            .run(tauri::generate_context!())
    });

    match result {
        Ok(app_result) => {
            if let Err(e) = app_result {
                eprintln!("❌ Tauri application error: {e}");
                std::process::exit(1);
            }
        }
        Err(panic_payload) => {
            eprintln!("❌ Application panicked during startup");
            if let Some(panic_str) = panic_payload.downcast_ref::<&str>() {
                eprintln!("   Panic message: {panic_str}");
            } else if let Some(panic_string) = panic_payload.downcast_ref::<String>() {
                eprintln!("   Panic message: {panic_string}");
            }

            eprintln!("💡 Troubleshooting suggestions:");
            eprintln!(
                "   1. Check WebKit/GTK installation: sudo apt install libwebkit2gtk-4.1-dev"
            );
            eprintln!("   2. Update graphics drivers");
            eprintln!("   3. Set WEBKIT_DISABLE_COMPOSITING_MODE=1");
            eprintln!("   4. Run in a desktop environment with proper display");

            std::process::exit(1);
        }
    }
}
