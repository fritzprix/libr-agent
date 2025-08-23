# MCP FilesystemServer 리팩토링 계획 (replace_lines_in_file 도구 추가)

## 작업의 목적
- MCP FilesystemServer에 파일의 특정 라인들을 한 번에 교체할 수 있는 범용 도구(replace_lines_in_file)를 추가하여, 파일 분석 및 자동 수정 기능을 강화한다.

## 현재의 상태 / 문제점
- 기존 MCP FilesystemServer에는 파일을 읽고 쓰는(read_file, write_file) 기본 도구만 존재하며, 특정 라인 교체와 같은 세밀한 파일 수정 기능이 없다.
- 파일 내 특정 패턴을 찾아 교체하거나, 여러 라인을 한 번에 수정하는 작업이 불편함.

## 변경 이후의 상태 / 해결 판정 기준
- MCP FilesystemServer에 replace_lines_in_file 도구가 추가되어, 여러 라인을 한 번에 교체할 수 있다.
- 도구는 path(파일 경로)와 replacements(교체할 라인 번호 및 내용 배열)를 입력받아, 지정된 라인들을 새로운 내용으로 교체한다.
- 정상적으로 동작하면, 파일의 지정된 라인들이 교체되고 성공 메시지를 반환한다.
- 에러 상황(존재하지 않는 라인, 파일 접근 오류 등)도 명확히 처리한다.

## 수정이 필요한 코드 및 수정부분의 코드 스니핏

### 1. MCPTool 정의 추가
```rust
fn create_replace_lines_in_file_tool() -> MCPTool {
  // ... replace_lines_in_file MCPTool 정의 ...
}
```

### 2. 핸들러 함수 추가
```rust
async fn handle_replace_lines_in_file(&self, args: Value) -> MCPResponse {
  // ... path, replacements 파싱 및 라인 교체 로직 ...
}
```

### 3. tools()에 도구 추가
```rust
fn tools(&self) -> Vec<MCPTool> {
  vec![
    // ...existing tools...
    Self::create_replace_lines_in_file_tool(),
  ]
}
```

### 4. call_tool()에 핸들러 연결
```rust
async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse {
  match tool_name {
    // ...existing tools...
    "replace_lines_in_file" => self.handle_replace_lines_in_file(args).await,
    _ => { /* ... */ }
  }
}
```
