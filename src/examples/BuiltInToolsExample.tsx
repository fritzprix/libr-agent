import { useState } from 'react';
import { useBuiltInTools } from '@/context/BuiltInToolContext';
import { MCPResponse } from '@/lib/mcp-types';

/**
 * Built-in 도구들의 사용 예시를 보여주는 컴포넌트
 */
export function BuiltInToolsExample() {
  const {
    availableTools,
    webWorkerTools,
    tauriBuiltinTools,
    isLoadingTauriTools,
    executeToolCall,
  } = useBuiltInTools();

  const [results, setResults] = useState<Record<string, MCPResponse>>({});
  const [isExecuting, setIsExecuting] = useState<Record<string, boolean>>({});

  const executeTool = async (toolName: string, args: unknown) => {
    const executionId = `${toolName}-${Date.now()}`;

    setIsExecuting((prev) => ({ ...prev, [executionId]: true }));

    try {
      const result = await executeToolCall({
        id: `test-${Date.now()}`,
        type: 'function',
        function: {
          name: toolName,
          arguments: JSON.stringify(args),
        },
      });

      setResults((prev) => ({ ...prev, [executionId]: result }));
    } catch (error) {
      console.error('Tool execution failed:', error);
      setResults((prev) => ({
        ...prev,
        [executionId]: {
          jsonrpc: '2.0',
          id: `test-${Date.now()}`,
          error: {
            code: -32603,
            message: error instanceof Error ? error.message : String(error),
          },
        },
      }));
    } finally {
      setIsExecuting((prev) => ({ ...prev, [executionId]: false }));
    }
  };

  const filesystemExamples = [
    {
      title: 'Read package.json',
      toolName: 'builtin:filesystem__read_file',
      args: { path: 'package.json' },
    },
    {
      title: 'List current directory',
      toolName: 'builtin:filesystem__list_directory',
      args: { path: '.' },
    },
    {
      title: 'Write test file',
      toolName: 'builtin:filesystem__write_file',
      args: {
        path: 'test-output.txt',
        content: `Test file created at ${new Date().toISOString()}`,
      },
    },
  ];

  const sandboxExamples = [
    {
      title: 'Python: Calculate factorial',
      toolName: 'builtin:sandbox__execute_python',
      args: {
        code: `
def factorial(n):
    if n <= 1:
        return 1
    return n * factorial(n - 1)

print(f"Factorial of 5: {factorial(5)}")
print(f"Factorial of 10: {factorial(10)}")
        `.trim(),
        timeout: 10,
      },
    },
    {
      title: 'TypeScript: Array operations',
      toolName: 'builtin:sandbox__execute_typescript',
      args: {
        code: `
const numbers = [1, 2, 3, 4, 5];
const doubled = numbers.map(n => n * 2);
const sum = numbers.reduce((a, b) => a + b, 0);

console.log('Original:', numbers);
console.log('Doubled:', doubled);
console.log('Sum:', sum);
        `.trim(),
        timeout: 10,
      },
    },
  ];

  return (
    <div className="p-6 space-y-6">
      <div className="border-b pb-4">
        <h1 className="text-2xl font-bold mb-2">
          Built-in Tools Integration Example
        </h1>
        <div className="text-sm text-gray-600 space-y-1">
          <p>Total available tools: {availableTools.length}</p>
          <p>Web Worker tools: {webWorkerTools.length}</p>
          <p>Tauri built-in tools: {tauriBuiltinTools.length}</p>
          {isLoadingTauriTools && (
            <p className="text-blue-600">Loading Tauri tools...</p>
          )}
        </div>
      </div>

      {/* Tool Categories */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Web Worker Tools */}
        <div className="space-y-4">
          <h2 className="text-lg font-semibold">Web Worker Tools</h2>
          <div className="text-sm text-gray-600">
            {webWorkerTools.length > 0 ? (
              <ul className="space-y-1">
                {webWorkerTools.map((tool) => (
                  <li key={tool.name} className="truncate">
                    {tool.name}
                  </li>
                ))}
              </ul>
            ) : (
              <p>No Web Worker tools available</p>
            )}
          </div>
        </div>

        {/* Tauri Built-in Tools */}
        <div className="space-y-4">
          <h2 className="text-lg font-semibold">Tauri Built-in Tools</h2>
          <div className="text-sm text-gray-600">
            {tauriBuiltinTools.length > 0 ? (
              <ul className="space-y-1">
                {tauriBuiltinTools.map((tool) => (
                  <li key={tool.name} className="truncate">
                    {tool.name}
                  </li>
                ))}
              </ul>
            ) : (
              <p>No Tauri built-in tools available</p>
            )}
          </div>
        </div>

        {/* All Tools */}
        <div className="space-y-4">
          <h2 className="text-lg font-semibold">All Available Tools</h2>
          <div className="text-sm text-gray-600 max-h-48 overflow-y-auto">
            {availableTools.length > 0 ? (
              <ul className="space-y-1">
                {availableTools.map((tool) => (
                  <li
                    key={tool.name}
                    className="truncate"
                    title={tool.description}
                  >
                    {tool.name}
                  </li>
                ))}
              </ul>
            ) : (
              <p>No tools available</p>
            )}
          </div>
        </div>
      </div>

      {/* Filesystem Examples */}
      <div className="space-y-4">
        <h2 className="text-lg font-semibold">Filesystem Tools Examples</h2>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          {filesystemExamples.map((example, index) => (
            <button
              key={index}
              onClick={() => executeTool(example.toolName, example.args)}
              disabled={Object.values(isExecuting).some(Boolean)}
              className="p-3 border rounded hover:bg-gray-50 disabled:opacity-50 text-left"
            >
              <div className="font-medium">{example.title}</div>
              <div className="text-sm text-gray-600 mt-1">
                {example.toolName.split('__')[1]}
              </div>
            </button>
          ))}
        </div>
      </div>

      {/* Sandbox Examples */}
      <div className="space-y-4">
        <h2 className="text-lg font-semibold">Sandbox Tools Examples</h2>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {sandboxExamples.map((example, index) => (
            <button
              key={index}
              onClick={() => executeTool(example.toolName, example.args)}
              disabled={Object.values(isExecuting).some(Boolean)}
              className="p-3 border rounded hover:bg-gray-50 disabled:opacity-50 text-left"
            >
              <div className="font-medium">{example.title}</div>
              <div className="text-sm text-gray-600 mt-1">
                {example.toolName.split('__')[1]}
              </div>
            </button>
          ))}
        </div>
      </div>

      {/* Results */}
      {Object.keys(results).length > 0 && (
        <div className="space-y-4">
          <h2 className="text-lg font-semibold">Execution Results</h2>
          <div className="space-y-4 max-h-96 overflow-y-auto">
            {Object.entries(results).map(([executionId, result]) => (
              <div key={executionId} className="border rounded p-4">
                <div className="text-sm text-gray-600 mb-2">
                  Execution ID: {executionId}
                </div>
                <pre className="text-xs bg-gray-100 p-2 rounded overflow-x-auto">
                  {JSON.stringify(result, null, 2)}
                </pre>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Loading indicator */}
      {Object.values(isExecuting).some(Boolean) && (
        <div className="fixed bottom-4 right-4 bg-blue-600 text-white px-4 py-2 rounded shadow">
          Executing tool...
        </div>
      )}
    </div>
  );
}

export default BuiltInToolsExample;
