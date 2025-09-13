# WorkspaceFilesPanel의 GUI 개선 및 Tauri MCP (BuiltIn 도구) Command 추가

## workspace 도구에 import_file 추가

- Input Parameter

  ```ts
  interface ImportFileInput {
    src_abs_path: string; // absolute path of source file to be imported into workspace
    dest_rel_path: string; // relative path of destination location where the file is imported to
  }
  ```

## WorkspaceFilesPanel의 Drop event 처리

- 파일 드랍 시 rust-backend-client.ts 등을 사용하여 위 tool을 호출
- 응답 까지 받은 후
- 도구 사용과 도구 응답을 메시지로 추가

- Simplified Snippet

  ```tsx
  const { submit } = useChatContext();
  const { callTool? } = useRustBackend(); // i'm not sure the exact signature of the interface though, there should be one that can call the builtin MCP tool implemented in rust backend

  ...

  const handleFileDrop = useCallback(async () => {
    const requestMessage = createToolMessage("import_file", {...});
    const responseMessage = callTool("import_file", {...})

    submit([requestMessage, responseMessage]);
  },[callTool, ...]);

  ```
