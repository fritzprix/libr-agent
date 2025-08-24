import { useState } from 'react';
import { createId } from '@paralleldrive/cuid2';
import { getLogger } from '@/lib/logger';
import type { MCPTool } from '@/lib/mcp-types';
import type { ToolCall } from '@/models/chat';
import { BuiltInToolProvider, useBuiltInTool } from '@/features/tools';
import { RustMCPToolProvider } from '@/features/tools/RustMCPToolProvider';

const logger = getLogger('RustMCPDemo');
// demo component for Rust MCP tools

function ToolListDemo() {
  const { availableTools, executeTool, status } = useBuiltInTool();
  const [output, setOutput] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const handleRunTool = async (tool: MCPTool) => {
    setLoading(true);
    const toolCall: ToolCall = {
      id: createId(),
      type: 'function',
      function: {
        name: tool.name,
        arguments: JSON.stringify({}), // Provide actual arguments if needed
      },
    };
    try {
      logger.debug('Executing Rust MCP tool', { tool: tool.name, toolCall });
      const result = await executeTool(toolCall);
      setOutput(JSON.stringify(result, null, 2));
      logger.info('Rust MCP tool executed', { tool: tool.name, result });
    } catch (err) {
      logger.error('Rust MCP tool execution failed', err);
      setOutput(`Error: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="p-4">
      <h2 className="font-bold mb-2">Rust MCP Available Tools</h2>
      <ul className="mb-4">
        {availableTools.length === 0 ? (
          <li className="text-muted-foreground">
            No Rust MCP tools available.
          </li>
        ) : (
          availableTools.map((tool) => (
            <li key={tool.name} className="mb-2 flex items-center">
              <span className="font-mono">{tool.name}</span>
              {tool.description && (
                <span className="ml-2 text-sm text-muted-foreground">
                  - {tool.description}
                </span>
              )}
              <button
                className="ml-auto px-2 py-1 bg-primary text-white rounded text-sm"
                disabled={loading}
                onClick={() => handleRunTool(tool)}
              >
                {loading ? 'Running...' : 'Run'}
              </button>
            </li>
          ))
        )}
      </ul>
      <div>
        <h3 className="font-bold mb-1">Tool Output</h3>
        <pre className="bg-muted p-2 rounded text-xs whitespace-pre-wrap max-h-96 overflow-y-auto">
          {output || 'No output yet. Run a tool to see results.'}
        </pre>
      </div>
      <div className="mt-4">
        <h4 className="font-bold">Service Status</h4>
        <pre className="bg-muted p-2 rounded text-xs">
          {JSON.stringify(status, null, 2)}
        </pre>
      </div>
    </div>
  );
}

export default function RustMCPDemo() {
  return (
    <BuiltInToolProvider>
      <RustMCPToolProvider />
      <ToolListDemo />
    </BuiltInToolProvider>
  );
}
