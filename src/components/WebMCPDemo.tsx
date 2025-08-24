import { useState } from 'react';
import { createId } from '@paralleldrive/cuid2';
import { getLogger } from '@/lib/logger';
import type { MCPTool } from '@/lib/mcp-types';
import type { ToolCall } from '@/models/chat';
import { BuiltInToolProvider, useBuiltInTool } from '@/features/tools';
import { WebMCPProvider } from '@/features/tools/WebMCPToolProvider';

const logger = getLogger('WebMCPDemo');
const DEMO_SERVERS = ['content-store'];

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
      logger.debug('Executing tool', { tool: tool.name, toolCall });
      const result = await executeTool(toolCall);
      setOutput(JSON.stringify(result, null, 2));
      logger.info('Tool executed', { tool: tool.name, result });
    } catch (err) {
      logger.error('Tool execution failed', err);
      setOutput(`Error: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="p-4">
      <h2 className="font-bold mb-2">Available Tools</h2>
      <ul className="mb-4">
        {availableTools.length === 0 ? (
          <li className="text-muted-foreground">No tools available.</li>
        ) : (
          availableTools.map((tool) => (
            <li key={tool.name} className="mb-2 flex items-center">
              <span className="font-mono">{tool.name}</span>
              <button
                className="ml-2 px-2 py-1 bg-primary text-white rounded"
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
        <pre className="bg-muted p-2 rounded text-xs whitespace-pre-wrap">
          {output}
        </pre>
      </div>
      <div className="mt-4">
        <h4 className="font-bold">Service Status</h4>
        <pre className="text-xs">{JSON.stringify(status, null, 2)}</pre>
      </div>
    </div>
  );
}

export default function WebMCPDemo() {
  return (
    <BuiltInToolProvider>
      <WebMCPProvider servers={DEMO_SERVERS} />
      <ToolListDemo />
    </BuiltInToolProvider>
  );
}
