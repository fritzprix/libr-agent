use std::sync::OnceLock;
use tauri_plugin_log::{Target, TargetKind};
use log::error;
mod mcp;
use mcp::{MCPServerConfig, MCPServerManager, ToolCallResult};

// 전역 MCP 서버 매니저
static MCP_MANAGER: OnceLock<MCPServerManager> = OnceLock::new();

fn get_mcp_manager() -> &'static MCPServerManager {
    MCP_MANAGER.get_or_init(|| MCPServerManager::new())
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
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
) -> ToolCallResult {
    get_mcp_manager()
        .call_tool(&server_name, &tool_name, arguments)
        .await
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
                    .map_err(|e| format!("Invalid server config: {}", e))?;
                server_list.push(server_cfg);
            }
            server_list
        } else if let Some(servers_array) = config.get("servers").and_then(|v| v.as_array()) {
            // 기존 format: servers 배열
            println!("🚀 [TAURI] Processing legacy format (servers array)");
            let mut server_list = Vec::new();
            for server_value in servers_array {
                let server_cfg: mcp::MCPServerConfig = serde_json::from_value(server_value.clone())
                    .map_err(|e| format!("Invalid server config: {}", e))?;
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
            println!("🚀 [TAURI] Starting server: {}", server_name);
            if let Err(e) = manager.start_server(server_cfg).await {
                eprintln!("❌ [TAURI] Failed to start server {}: {}", server_name, e);
                continue; // Skip to the next server if this one fails to start
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        } else {
            println!("🚀 [TAURI] Server {} already running", server_name);
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
                eprintln!(
                    "❌ [TAURI] Error listing tools for '{}': {}",
                    server_name, e
                );
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
    mcp::MCPServerManager::validate_tool_schema(&tool)
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Set up custom panic handler for better error reporting
    std::panic::set_hook(Box::new(|panic_info| {
        error!("🚨 PANIC: {}", panic_info);
        if let Some(location) = panic_info.location() {
            error!("  Location: {}:{}:{}", location.file(), location.line(), location.column());
        }
        
        // Attempt graceful shutdown
        error!("🔄 Attempting graceful shutdown...");
    }));

    // Configure Tauri builder with error handling
    let result = std::panic::catch_unwind(|| {
        tauri::Builder::default()
            .plugin(
                tauri_plugin_log::Builder::new()
                    .targets([
                        Target::new(TargetKind::Stdout),
                        Target::new(TargetKind::LogDir { file_name: None }),
                        Target::new(TargetKind::Webview),
                    ])
                    .build(),
            )
            .plugin(tauri_plugin_opener::init())
            .invoke_handler(tauri::generate_handler![
                greet,
                start_mcp_server,
                stop_mcp_server,
                call_mcp_tool,
                list_mcp_tools,
                list_tools_from_config,
                get_connected_servers,
                check_server_status,
                check_all_servers_status,
                list_all_tools,
                get_validated_tools,
                validate_tool_schema
            ])
            .setup(|_app| {
                println!("🚀 SynapticFlow initializing...");
                
                // Verify WebView can be created safely
                #[cfg(target_os = "linux")]
                {
                    println!("🐧 Linux detected - checking WebKit compatibility...");
                    
                    // Set environment variables for better WebKit compatibility
                    std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
                    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
                    
                    // Check if running in a container or limited environment
                    if std::env::var("container").is_ok() || 
                       std::env::var("DISPLAY").is_err() {
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
                eprintln!("❌ Tauri application error: {}", e);
                std::process::exit(1);
            }
        }
        Err(panic_payload) => {
            eprintln!("❌ Application panicked during startup");
            if let Some(panic_str) = panic_payload.downcast_ref::<&str>() {
                eprintln!("   Panic message: {}", panic_str);
            } else if let Some(panic_string) = panic_payload.downcast_ref::<String>() {
                eprintln!("   Panic message: {}", panic_string);
            }
            
            eprintln!("💡 Troubleshooting suggestions:");
            eprintln!("   1. Check WebKit/GTK installation: sudo apt install libwebkit2gtk-4.1-dev");
            eprintln!("   2. Update graphics drivers");
            eprintln!("   3. Set WEBKIT_DISABLE_COMPOSITING_MODE=1");
            eprintln!("   4. Run in a desktop environment with proper display");
            
            std::process::exit(1);
        }
    }
}
