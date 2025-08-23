# Browser Agent Server HTML 내용 추출 구현 계획

## 작업의 목적

`BrowserAgentServer`에서 실제 웹페이지 HTML 내용을 올바르게 추출하여 크롤링 기능을 완전히 구현합니다.

## 현재의 상태 / 문제점

### 1. **핵심 문제: `execute_script` 메서드의 JavaScript 결과 처리 실패**

**파일**: `src-tauri/src/services/interactive_browser_server.rs` (lines 130-150)

```rust
match window.eval(script) {
    Ok(result) => {
        debug!("Script executed successfully in session: {}", session_id);
        
        // ❌ 문제: format!("{:?}", result)로 debug representation만 사용
        let result_string = format!("{:?}", result);
        
        // ❌ 복잡한 문자열 파싱이 대부분 실패
        if result_string.contains("String(") {
            if let Some(start) = result_string.find("String(\"") {
                if let Some(end) = result_string[start + 8..].find("\")") {
                    let content = &result_string[start + 8..start + 8 + end];
                    return Ok(content.to_string());
                }
            }
        }
        
        // ❌ 결국 debug representation 반환
        Ok(result_string)
    }
}
```

- `window.eval(script)`의 실제 반환값을 `format!("{:?}", result)`로 처리
- 복잡한 문자열 파싱 로직이 대부분 실패
- **실제 HTML이나 JSON 데이터 대신 debug string만 반환**

### 2. **Data Extraction 결과가 저장되지 않음**

- `extract_data` 툴 실행 시 **결과가 MCP 응답에만 포함됨**
- **파일이나 영구 저장소에 저장되지 않음**
- 단순히 "✅ Data extraction successful" 메시지만 반환

### 3. `get_page_content`가 실제 HTML을 반환하지 못함

**파일**: `src-tauri/src/services/interactive_browser_server.rs` (lines 333-337)

```rust
pub async fn get_page_content(&self, session_id: &str) -> Result<String, String> {
    debug!("Getting page content for session {}", session_id);
    let script = "document.documentElement.outerHTML";
    self.execute_script(session_id, script).await  // ❌ debug string만 반환
}
```

- `document.documentElement.outerHTML` 실행하지만 실제 HTML 대신 debug string 반환
- 저장된 모든 HTML 파일에 실제 웹페이지 내용이 아닌 debug 정보만 포함

## 변경 이후의 상태 / 해결 판정 기준

### 성공 기준

1. **실제 웹페이지 HTML 추출**: 크롤링 결과에 실제 DOM 구조가 저장됨
2. **`extract_data` 툴 동작**: JavaScript 코드 실행 및 결과 반환 가능
3. **구조화된 데이터 추출**: CSS 셀렉터를 통한 특정 데이터 추출 성공
4. **테스트 검증**: Hacker News 등에서 실제 데이터 추출 성공

### 검증 방법

- Hacker News에서 "Show HN" 상위 10개 토픽 제목과 링크 추출
- 저장된 HTML 파일에 실제 웹페이지 내용이 포함되는지 확인
- `extract_data` 툴로 JavaScript 코드 실행이 정상 동작하는지 확인

## 수정이 필요한 코드 및 수정부분의 코드 스니핏

### 1. **가장 중요한 수정: InteractiveBrowserServer::execute_script 메서드**

**파일**: `src-tauri/src/services/interactive_browser_server.rs` (lines 112-132)

```rust
// 현재 상태 - 문제점
pub async fn execute_script(&self, session_id: &str, script: &str) -> Result<String, String> {
    // ...
    if let Some(window) = self.app_handle.get_webview_window(&session.window_label) {
        match window.eval(script) {
            Ok(_) => {  // ❌ 실제 결과를 무시
                debug!("Script executed successfully in session: {}", session_id);
                Ok("Script executed successfully".to_string())  // ❌ 하드코딩
            }
            Err(e) => {
                error!("Failed to execute script in session {}: {}", session_id, e);
                Err(format!("Failed to execute script: {}", e))
            }
        }
    }
    // ...
}

// 수정 후 - 실제 JavaScript 결과 반환
pub async fn execute_script(&self, session_id: &str, script: &str) -> Result<String, String> {
    debug!("Executing script in session {}: {}", session_id, script);

    let session = {
        let sessions = self
            .sessions
            .read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        sessions
            .get(session_id)
            .cloned()
            .ok_or("Session not found")?
    };

    if let Some(window) = self.app_handle.get_webview_window(&session.window_label) {
        match window.eval(script) {
            Ok(result) => {
                // ✅ 실제 JavaScript 실행 결과를 반환
                debug!("Script executed successfully in session: {}", session_id);
                
                // Tauri의 eval 결과를 문자열로 변환
                let result_string = match result {
                    tauri::api::Result::Ok(value) => {
                        // JSON 값을 문자열로 변환
                        match value {
                            serde_json::Value::String(s) => s,
                            other => other.to_string(),
                        }
                    }
                    tauri::api::Result::Err(e) => {
                        return Err(format!("JavaScript execution error: {}", e));
                    }
                };
                
                Ok(result_string)
            }
            Err(e) => {
                error!("Failed to execute script in session {}: {}", session_id, e);
                Err(format!("Failed to execute script: {}", e))
            }
        }
    } else {
        error!("Browser window not found for session: {}", session_id);
        Err("Browser window not found".to_string())
    }
}
```

### 2. get_page_content 메서드는 이미 올바르게 구현됨

**파일**: `src-tauri/src/services/interactive_browser_server.rs` (lines 333-337)

```rust
// 현재 상태 - 이미 올바르게 구현되어 있음
pub async fn get_page_content(&self, session_id: &str) -> Result<String, String> {
    debug!("Getting page content for session {}", session_id);
    let script = "document.documentElement.outerHTML";
    self.execute_script(session_id, script).await  // ✅ execute_script 수정 후 정상 동작할 것
}
```

- `get_page_content` 자체는 문제없이 구현되어 있음
- `execute_script` 메서드 수정 후 자동으로 정상 동작할 것

### 3. BrowserAgentServer extract_data 툴 구현

**파일**: `src-tauri/src/mcp/builtin/browser_agent_server.rs`

```rust
// 현재 상태
async fn handle_extract_data(&self, args: Value) -> MCPResponse {
    MCPResponse::error(
        -32601,
        "Extract data functionality not yet implemented".to_string(),
        None,
    )
}

// 수정 후
async fn handle_extract_data(&self, args: Value) -> MCPResponse {
    let request_id = Value::String(Uuid::new_v4().to_string());

    let session_id = match args.get("session_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => {
            return MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: None,
                error: Some(MCPError {
                    code: -32602,
                    message: "Missing required parameter: session_id".to_string(),
                    data: None,
                }),
            };
        }
    };

    let script = match args.get("script").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: None,
                error: Some(MCPError {
                    code: -32602,
                    message: "Missing required parameter: script".to_string(),
                    data: None,
                }),
            };
        }
    };

    match self.browser_server.execute_script(session_id, script).await {
        Ok(result) => {
            // JSON 파싱 시도
            let parsed_result = match serde_json::from_str::<Value>(&result) {
                Ok(json_val) => json_val,
                Err(_) => json!(result), // 파싱 실패 시 문자열로 처리
            };

            MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: Some(json!({
                    "content": [{
                        "type": "text",
                        "text": format!("✅ Data extraction successful from session: {}", session_id)
                    }],
                    "data": {
                        "session_id": session_id,
                        "script": script,
                        "result": parsed_result
                    }
                })),
                error: None,
            }
        }
        Err(e) => MCPResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(request_id),
            result: None,
            error: Some(MCPError {
                code: -32603,
                message: format!("Script execution failed: {}", e),
                data: None,
            }),
        },
    }
}
```

### 4. 페이지 로딩 대기 개선

**파일**: `src-tauri/src/mcp/builtin/browser_agent_server.rs`

```rust
// perform_advanced_crawl 메서드의 HTML 내용 추출 부분 수정
// 현재 상태 (lines 147-156)
let html_content = match self.browser_server.get_page_content(&session_id).await {
    Ok(content) => content,
    Err(e) => {
        warn!("Failed to get page content: {}", e);
        String::new()
    }
};

// 수정 후
// 페이지 로딩 대기 (더 길게)
if wait_for_networkidle {
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
} else {
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
}

// DOM 준비 상태 확인
let ready_check = r#"
    document.readyState === 'complete' && 
    document.body && 
    document.body.children.length > 0
"#;

match self.browser_server.execute_script(&session_id, ready_check).await {
    Ok(ready_state) => {
        info!("Page ready state: {}", ready_state);
        if ready_state.contains("false") {
            warn!("Page not fully loaded, waiting additional 2 seconds");
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    }
    Err(e) => warn!("Failed to check page ready state: {}", e),
}

// HTML 내용 추출 - 재시도 로직 추가
let html_content = match self.browser_server.get_page_content(&session_id).await {
    Ok(content) => {
        if content.is_empty() || content == "Script executed successfully" {
            warn!("Received empty or default content, retrying...");
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            
            match self.browser_server.get_page_content(&session_id).await {
                Ok(retry_content) => {
                    if retry_content.is_empty() || retry_content == "Script executed successfully" {
                        warn!("Retry also failed, using empty content");
                        String::new()
                    } else {
                        info!("Retry successful, got {} characters", retry_content.len());
                        retry_content
                    }
                }
                Err(e) => {
                    warn!("Retry failed: {}", e);
                    String::new()
                }
            }
        } else {
            info!("Successfully got page content: {} characters", content.len());
            content
        }
    }
    Err(e) => {
        warn!("Failed to get page content: {}", e);
        String::new()
    }
};
```

### 5. MCP 툴 정의에 extract_data 추가

**파일**: `src-tauri/src/mcp/builtin/browser_agent_server.rs`

```rust
// get_tools 메서드에 extract_data 툴 추가
fn get_extract_data_tool() -> MCPTool {
    MCPTool {
        name: "extract_data".to_string(),
        description: "Execute JavaScript code in a browser session and extract data".to_string(),
        input_schema: JSONSchema {
            schema_type: JSONSchemaType::Object,
            properties: Some({
                let mut props = HashMap::new();
                props.insert("session_id".to_string(), JSONSchema {
                    schema_type: JSONSchemaType::String,
                    description: Some("Browser session ID".to_string()),
                    ..Default::default()
                });
                props.insert("script".to_string(), JSONSchema {
                    schema_type: JSONSchemaType::String,
                    description: Some("JavaScript code to execute for data extraction".to_string()),
                    ..Default::default()
                });
                props
            }),
            required: Some(vec!["session_id".to_string(), "script".to_string()]),
            ..Default::default()
        },
    }
}

// get_tools 메서드에서 extract_data 툴 포함
pub fn get_tools(&self) -> Vec<MCPTool> {
    vec![
        // ...existing tools...
        Self::get_extract_data_tool(),
    ]
}
```

## 구현 순서

1. **가장 중요: InteractiveBrowserServer::execute_script 메서드 수정** (JavaScript 결과 올바르게 반환)
2. **BrowserAgentServer extract_data 툴 구현**
3. **페이지 로딩 대기 로직 개선** (선택사항)
4. **MCP 툴 정의 업데이트**
5. **테스트 및 검증**

## 예상 소요 시간

- **InteractiveBrowserServer execute_script 수정: 1-2시간** (가장 중요)
- BrowserAgentServer 툴 구현: 2-3시간
- 페이지 로딩 로직 개선: 1-2시간 (선택사항)
- 테스트 및 디버깅: 1-2시간
- **총 예상 시간: 5-9시간** (기존 7-11시간에서 단축)

## 위험 요소

- **Tauri webview eval 메서드 반환 타입**: 정확한 API 문서 확인 필요
- **JavaScript 결과 직렬화**: 복잡한 객체의 문자열 변환 처리
- **세션 상태 관리**: 동시 다발적 크롤링 시 세션 상태 충돌 가능성

## 추가 고려사항

- Tauri 1.x의 `window.eval()` 반환 타입 정확히 파악 필요
- JavaScript 실행 결과의 JSON 직렬화 처리 방법 결정
- 에러 처리 및 로깅 강화로 디버깅 용이성 향상