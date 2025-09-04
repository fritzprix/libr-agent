# Workspace Export 기능 추가 (MCP-UI 기반) Refactoring Plan

## 작업의 목적

기존 WorkspaceServer에 **MCP-UI UIResource 기반 export 기능**을 추가하여 사용자가 작업 결과물을 직관적이고 즉시 다운로드할 수 있는 인터랙티브 UI를 제공한다.

### 핵심 목표

- **MCP-UI UIResource 응답**: export 도구가 클릭 가능한 다운로드 버튼이 포함된 HTML UI를 반환
- **개별 파일 Export**: workspace 내 단일 파일을 exports 디렉토리에 복사 후 다운로드 UI 제공
- **ZIP 패키지 Export**: 여러 파일을 ZIP으로 묶어 다운로드 UI 제공
- **HTML 기반 간소화**: Remote-DOM 대신 표준 HTML UIResource 사용으로 복잡도 최소화
- **기존 아키텍처 활용**: 현재 MCP-UI 인프라(@mcp-ui/client ^5.6.2)와 SecureFileManager를 최대한 재사용

## 현재의 상태 / 문제점

### 1. Export 기능 부재

- **WorkspaceServer**: 파일 읽기/쓰기/실행 기능만 제공, export 기능 없음
- **텍스트 기반 응답**: MCP Tool 응답이 단순 텍스트로만 제공되어 사용자 액션 불가
- **수동 다운로드 프로세스**: 사용자가 별도 UI를 통해 파일을 찾고 다운로드해야 함
- **워크플로우 단절**: 파일 생성 → 분석 → 결과 활용의 자연스러운 흐름 부족

### 2. MCP-UI 활용 부족

- **정적 응답**: 현재 MCP Tool은 텍스트만 반환, 인터랙티브 요소 없음
- **UIResource 미활용**: MCP-UI의 핵심 기능인 UIResource 기반 리치 UI 응답 미사용
- **직접 액션 불가**: 응답 내에서 바로 다운로드하거나 추가 작업 수행 불가능

### 3. 파일 관리 구조 부족

- **Export 디렉토리 없음**: workspace에 exports 전용 디렉토리 구조 부재
- **ZIP 생성 불가**: `zip` crate 의존성 없어 패키지 생성 불가
- **메타데이터 생성 없음**: 자동 문서화 및 export 정보 제공 기능 부재

## 추가 분석 과제

### 1. 다운로드 트리거 메커니즘 분석

- **Tauri Asset Protocol**: `tauri://localhost/workspace/` 스킴을 사용한 직접 다운로드 링크
- **보안 정책 검증**: MCP-UI iframe에서 Tauri asset 접근 가능성 확인
- **대안 메커니즘**: 직접 접근 불가 시 postMessage 기반 우회 방안 검토

### 2. HTML UIResource vs Remote-DOM 방식 선택

- **HTML UIResource**: 표준 HTML/CSS/JS로 구현, 기존 MCP-UI 인프라 활용
- **Remote-DOM**: React 컴포넌트 기반 네이티브 스타일 UI (복잡도 증가)
- **선택**: **HTML UIResource** - 구현 단순성과 즉시 적용 가능성 우선

### 3. Export 파일 관리 정책

- **생명주기 관리**: export된 파일의 자동 정리 주기 및 정책
- **용량 제한**: workspace exports 디렉토리 최대 용량 및 모니터링
- **충돌 방지**: 동일 파일명 export 시 처리 방안

## 변경 이후의 상태 / 해결 판정 기준

### 1. MCP-UI UIResource 기반 Export 도구

- `export_file`: 단일 파일 export UI 제공
- `export_zip`: ZIP 패키지 export UI 제공
- 두 도구 모두 클릭 가능한 다운로드 링크가 포함된 HTML UIResource 반환

### 2. 향상된 사용자 경험

- **인터랙티브 UI**: 파일 목록, 메타데이터, 다운로드 버튼이 포함된 리치 UI
- **원클릭 다운로드**: UI 내 링크 클릭으로 즉시 다운로드 실행
- **시각적 피드백**: 현대적인 그라디언트 UI와 호버 효과로 사용자 경험 향상

### 3. 향상된 Workspace 구조

```bash
workspace/
├── exports/              # 새로 추가
│   ├── files/           # 개별 export 파일
│   └── packages/        # ZIP 패키지
├── scripts/
├── data/
└── temp/
```

### 4. 판정 기준

- [ ] MCP-UI UIResource 응답 구현 완료
- [ ] ZIP 생성 라이브러리 의존성 추가 완료
- [ ] export_file, export_zip MCP Tool이 HTML UIResource 반환
- [ ] UI에서 다운로드 링크 클릭 시 정상 다운로드 동작
- [ ] Tauri Asset Protocol을 통한 다운로드 검증
- [ ] 메타데이터 자동 생성 기능 동작
- [ ] exports 디렉토리 자동 생성 및 관리

## 수정이 필요한 코드 및 수정부분

### 1. 의존성 추가

**파일**: `src-tauri/Cargo.toml`

```toml
[dependencies]
# ...기존 의존성들...
zip = "0.6"
chrono = { version = "0.4", features = ["serde"] }  # 이미 있음
serde_json = "1.0"  # UIResource 생성용
```

### 2. WorkspaceServer에 HTML 기반 Export Tools 추가

**파일**: `src-tauri/src/mcp/builtin/workspace.rs`

```rust
// 기존 imports에 추가
use std::io::Write;
use zip::write::FileOptions;
use serde_json::json;

impl WorkspaceServer {
    // 기존 코드...

    /// UIResource 생성 (MCP-UI 표준 준수)
    fn create_export_ui_resource(
        &self,
        request_id: u64,
        title: &str,
        files: &[String],
        export_type: &str,
        download_path: &str,
        content: String,
    ) -> Value {
        // MCP-UI 표준 UIResource 형식
        json!({
            "type": "resource",
            "resource": {
                "uri": format!("ui://export/{}/{}", export_type.to_lowercase(), request_id),
                "mimeType": "text/html",
                "text": content,
                "title": title,
                "annotations": {
                    "export_type": export_type,
                    "file_count": files.len(),
                    "download_path": download_path,
                    "created_at": chrono::Utc::now().to_rfc3339()
                }
            }
        })
    }

    /// HTML UIResource 생성 (Tauri Asset Protocol 사용)
    fn create_html_export_ui(
        &self,
        title: &str,
        files: &[String],
        export_type: &str,
        download_path: &str,
        display_name: &str,
    ) -> String {
        let files_list = files
            .iter()
            .map(|f| format!("<li class='file-item'>{}</li>", f))
            .collect::<Vec<_>>()
            .join("");

        format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            max-width: 600px;
            margin: 0 auto;
            padding: 20px;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            min-height: 100vh;
        }}
        .container {{
            background: rgba(255, 255, 255, 0.1);
            border-radius: 15px;
            padding: 30px;
            backdrop-filter: blur(10px);
            box-shadow: 0 8px 32px 0 rgba(31, 38, 135, 0.37);
        }}
        h1 {{
            text-align: center;
            margin-bottom: 30px;
            font-size: 2em;
            text-shadow: 2px 2px 4px rgba(0,0,0,0.3);
        }}
        .export-info {{
            background: rgba(255, 255, 255, 0.1);
            border-radius: 10px;
            padding: 20px;
            margin-bottom: 30px;
        }}
        .download-btn {{
            background: linear-gradient(45deg, #2196F3, #21CBF3);
            color: white;
            border: none;
            padding: 15px 30px;
            font-size: 18px;
            border-radius: 25px;
            cursor: pointer;
            transition: all 0.3s ease;
            box-shadow: 0 4px 15px 0 rgba(33, 150, 243, 0.3);
            text-decoration: none;
            display: inline-block;
        }}
        .download-btn:hover {{
            transform: translateY(-2px);
            box-shadow: 0 6px 20px 0 rgba(33, 150, 243, 0.5);
        }}
        ul {{ padding: 0; list-style: none; }}
        .file-item {{
            background: rgba(255, 255, 255, 0.1);
            margin: 5px 0;
            padding: 8px 12px;
            border-radius: 5px;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>🎉 {}</h1>
        <div class="export-info">
            <h3>📦 Export Type: {}</h3>
            <p>📅 Created: {}</p>
            <p>📁 Files: {} items</p>
            <h4>📋 Included Files:</h4>
            <ul>{}</ul>
        </div>
        <div style="text-align: center;">
            <a href="tauri://localhost/workspace/{}" download="{}" class="download-btn">
                ⬇️ Download Now
            </a>
        </div>
    </div>
</body>
</html>"#,
            title, title, export_type,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            files.len(), files_list, download_path, display_name
        )
    }

    /// Export 디렉토리 초기화
    fn ensure_exports_directory(&self) -> Result<std::path::PathBuf, String> {
        let exports_dir = self.get_workspace_dir().join("exports");

        // files 및 packages 하위 디렉토리 생성
        let files_dir = exports_dir.join("files");
        let packages_dir = exports_dir.join("packages");

        for dir in [&exports_dir, &files_dir, &packages_dir] {
            if !dir.exists() {
                std::fs::create_dir_all(dir)
                    .map_err(|e| format!("Failed to create directory {:?}: {}", dir, e))?;
            }
        }

        Ok(exports_dir)
    }

    async fn handle_export_file(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        // 파라미터 파싱
        let path = args["path"]
            .as_str()
            .ok_or_else(|| "Missing required parameter: path")?;
        let display_name = args["display_name"]
            .as_str()
            .unwrap_or(path)
            .to_string();
        let description = args["description"]
            .as_str()
            .unwrap_or("")
            .to_string();

        // 소스 파일 경로 검증
        let source_path = self.get_workspace_dir().join(path);
        if !source_path.exists() || !source_path.is_file() {
            return Self::error_response(request_id, "File not found or is not a regular file");
        }

        // exports 디렉토리 준비
        let exports_dir = match self.ensure_exports_directory() {
            Ok(dir) => dir,
            Err(e) => return Self::error_response(request_id, &e),
        };

        // 고유한 export 파일명 생성
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let file_stem = source_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let file_ext = source_path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let export_filename = if file_ext.is_empty() {
            format!("{}_{}", file_stem, timestamp)
        } else {
            format!("{}_{}.{}", file_stem, timestamp, file_ext)
        };

        // 파일 복사
        let export_path = exports_dir.join("files").join(&export_filename);
        if let Err(e) = std::fs::copy(&source_path, &export_path) {
            return Self::error_response(request_id, &format!("Failed to copy file: {}", e));
        }

        let relative_path = format!("exports/files/{}", export_filename);
        let source_path_str = path;

        // HTML UIResource 생성
        let html_content = self.create_html_export_ui(
            &format!("File Export: {}", display_name),
            &[source_path_str.to_string()],
            "Single File",
            &relative_path,
            &display_name,
        );

        let ui_resource = self.create_export_ui_resource(
            request_id,
            &format!("File Export: {}", display_name),
            &[source_path_str.to_string()],
            "Single File",
            &relative_path,
            html_content,
        );

        Self::success_response_with_content(request_id, ui_resource)
    }

    async fn handle_export_zip(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        // 파라미터 파싱
        let files_array = args["files"]
            .as_array()
            .ok_or_else(|| "Missing required parameter: files (array)")?;
        let package_name = args["package_name"]
            .as_str()
            .unwrap_or("workspace_export")
            .to_string();
        let description = args["description"]
            .as_str()
            .unwrap_or("")
            .to_string();

        if files_array.is_empty() {
            return Self::error_response(request_id, "Files array cannot be empty");
        }

        // exports 디렉토리 준비
        let exports_dir = match self.ensure_exports_directory() {
            Ok(dir) => dir,
            Err(e) => return Self::error_response(request_id, &e),
        };

        // 고유한 ZIP 파일명 생성
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let zip_filename = format!("{}_{}.zip", package_name, timestamp);
        let zip_path = exports_dir.join("packages").join(&zip_filename);

        // ZIP 파일 생성
        let zip_file = match std::fs::File::create(&zip_path) {
            Ok(file) => file,
            Err(e) => return Self::error_response(request_id, &format!("Failed to create ZIP file: {}", e)),
        };

        let mut zip = zip::ZipWriter::new(zip_file);
        let options = FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        // 파일들을 ZIP에 추가
        let mut processed_files = Vec::new();
        for file_value in files_array {
            let file_path = match file_value.as_str() {
                Some(path) => path,
                None => continue,
            };

            let source_path = self.get_workspace_dir().join(file_path);
            if !source_path.exists() || !source_path.is_file() {
                continue; // 존재하지 않는 파일은 건너뛰기
            }

            // ZIP 내부 경로 설정 (디렉토리 구조 유지)
            let archive_path = file_path.replace("\\", "/");

            match zip.start_file(&archive_path, options) {
                Ok(_) => {},
                Err(e) => {
                    eprintln!("Failed to start file in ZIP: {}", e);
                    continue;
                }
            }

            match std::fs::read(&source_path) {
                Ok(content) => {
                    if let Err(e) = zip.write_all(&content) {
                        eprintln!("Failed to write file content to ZIP: {}", e);
                        continue;
                    }
                    processed_files.push(file_path.to_string());
                },
                Err(e) => {
                    eprintln!("Failed to read file {}: {}", file_path, e);
                    continue;
                }
            }
        }

        // ZIP 파일 완료
        if let Err(e) = zip.finish() {
            return Self::error_response(request_id, &format!("Failed to finalize ZIP: {}", e));
        }

        if processed_files.is_empty() {
            return Self::error_response(request_id, "No files were successfully added to ZIP");
        }

        let relative_path = format!("exports/packages/{}", zip_filename);

        // HTML UIResource 생성
        let html_content = self.create_html_export_ui(
            &format!("ZIP Package: {}", package_name),
            &processed_files,
            "ZIP Package",
            &relative_path,
            &zip_filename,
        );

        let ui_resource = self.create_export_ui_resource(
            request_id,
            &format!("ZIP Package: {}", package_name),
            &processed_files,
            "ZIP Package",
            &relative_path,
            html_content,
        );

        Self::success_response_with_content(request_id, ui_resource)
    }

    fn create_export_file_tool() -> MCPTool {
        let mut props = HashMap::new();
        props.insert(
            "path".to_string(),
            string_prop(Some(1), Some(1000), Some("Workspace 내 export할 파일 경로")),
        );
        props.insert(
            "display_name".to_string(),
            string_prop(None, None, Some("다운로드시 표시할 파일명 (선택적)")),
        );
        props.insert(
            "description".to_string(),
            string_prop(None, None, Some("파일 설명 (선택적)")),
        );

        MCPTool {
            name: "export_file".to_string(),
            title: Some("Export Single File".to_string()),
            description: "Export a single file from workspace for download with interactive UI".to_string(),
            input_schema: JSONSchema::Object {
                properties: props,
                required: vec!["path".to_string()],
                additional_properties: None,
            },
        }
    }

    fn create_export_zip_tool() -> MCPTool {
        let mut props = HashMap::new();
        props.insert(
            "files".to_string(),
            JSONSchema::Array {
                items: Some(Box::new(string_prop(Some(1), Some(1000), None))),
                min_items: Some(1),
                max_items: Some(100),
                description: Some("Export할 파일 경로들의 배열".to_string()),
            },
        );
        props.insert(
            "package_name".to_string(),
            string_prop(None, Some(50), Some("ZIP 패키지명 (선택적, 기본값: workspace_export)")),
        );
        props.insert(
            "description".to_string(),
            string_prop(None, None, Some("패키지 설명 (선택적)")),
        );

        MCPTool {
            name: "export_zip".to_string(),
            title: Some("Export ZIP Package".to_string()),
            description: "Export multiple files as a ZIP package for download with interactive UI".to_string(),
            input_schema: JSONSchema::Object {
                properties: props,
                required: vec!["files".to_string()],
                additional_properties: None,
            },
        }
    }
}
```

### 3. Tool 등록 추가

**파일**: `src-tauri/src/mcp/builtin/workspace.rs` (get_tools 메서드 수정)

```rust
impl MCPServer for WorkspaceServer {
    async fn get_tools(&self) -> Vec<MCPTool> {
        vec![
            // 기존 도구들...
            Self::create_list_files_tool(),
            Self::create_read_file_tool(),
            Self::create_write_file_tool(),
            Self::create_run_command_tool(),

            // 새로 추가
            Self::create_export_file_tool(),
            Self::create_export_zip_tool(),
        ]
    }

    async fn call_tool(&self, request: MCPRequest) -> MCPResponse {
        match request.params.name.as_str() {
            // 기존 핸들러들...
            "list_files" => self.handle_list_files(request.params.arguments).await,
            "read_file" => self.handle_read_file(request.params.arguments).await,
            "write_file" => self.handle_write_file(request.params.arguments).await,
            "run_command" => self.handle_run_command(request.params.arguments).await,

            // 새로 추가
            "export_file" => self.handle_export_file(request.params.arguments).await,
            "export_zip" => self.handle_export_zip(request.params.arguments).await,

            _ => Self::error_response(request.id, "Unknown tool"),
        }
    }
}
```

### 4. Tauri Asset Protocol 설정 확인

**파일**: `src-tauri/tauri.conf.json`

```json
{
  "tauri": {
    "security": {
      "assetProtocol": {
        "enable": true,
        "scope": ["$RESOURCE/**", "workspace/**"]
      }
    }
  }
}
```

## 구현 순서

### Phase 1: 기본 인프라 구축

1. **의존성 추가** (src-tauri/Cargo.toml)
   - `zip = "0.6"` 추가
   - 빌드 및 의존성 확인

2. **Export 디렉토리 구조 생성**
   - `ensure_exports_directory()` 메서드 구현
   - `workspace/exports/files/`, `workspace/exports/packages/` 자동 생성

3. **HTML UIResource 템플릿 구현**
   - `create_html_export_ui()` 메서드 구현
   - 모던 CSS 스타일링 적용

### Phase 2: Export 도구 구현

4. **단일 파일 Export**
   - `handle_export_file()` 메서드 구현
   - `create_export_file_tool()` 도구 정의

5. **ZIP 패키지 Export**
   - `handle_export_zip()` 메서드 구현
   - `create_export_zip_tool()` 도구 정의

6. **MCP Server 통합**
   - `get_tools()` 및 `call_tool()` 메서드에 새 도구 등록

### Phase 3: 테스트 및 검증

7. **기능 테스트**
   - 개별 파일 export 및 다운로드 테스트
   - ZIP 패키지 생성 및 다운로드 테스트
   - Tauri Asset Protocol 동작 검증

8. **UI/UX 검증**
   - MCP-UI iframe 내 HTML 렌더링 확인
   - 다운로드 링크 클릭 동작 검증
   - 에러 케이스 처리 확인

## 예상 결과

### 1. 사용자 경험 개선

- **시각적 피드백**: 현대적인 그라디언트 UI로 export 진행 상황 확인
- **원클릭 다운로드**: HTML 내 다운로드 링크 클릭으로 즉시 파일 다운로드
- **메타데이터 제공**: 파일 목록, 생성 시간, export 타입 등 상세 정보 표시

### 2. 기술적 장점

- **표준 호환성**: MCP-UI HTML UIResource 표준 준수로 안정성 확보
- **구현 단순성**: Remote-DOM 대신 표준 HTML 사용으로 복잡도 최소화
- **확장성**: ZIP 구조와 메타데이터로 향후 기능 확장 용이

### 3. 워크플로우 향상

```
1. AI 에이전트가 파일 생성/수정
   ↓
2. export_file 또는 export_zip 도구 호출
   ↓
3. MCP-UI에 인터랙티브 다운로드 UI 표시
   ↓
4. 사용자 클릭으로 즉시 다운로드 실행
```

이 계획을 통해 SynapticFlow의 MCP-UI 기반 export 기능이 완전히 구현되어 사용자가 AI 에이전트 작업 결과물을 즉시 다운로드할 수 있는 매끄러운 워크플로우를 제공하게 됩니다.
