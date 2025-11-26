import { invoke } from '@tauri-apps/api/core';

async function testKoreanInlineExecution() {
  console.log('Testing Korean inline execution...');
  try {
    const result = await invoke('mcp_execute_tool', {
      serverName: 'builtin_workspace',
      toolName: 'execute_windows_cmd',
      arguments: {
        command: 'python -c "print(\'안녕하세요\')"'
      }
    });
    console.log('Result:', result);
  } catch (error) {
    console.error('Error:', error);
  }
}

testKoreanInlineExecution();
