import React from 'react';
import { AgentToolCallGroup } from '../../AgentToolCallGroup';
import type { MCPToolCallContent } from '@/lib/mcp';
import type { Message } from '@/models/chat';

interface ToolGroupContentProps {
  items: MCPToolCallContent[];
  message: Message | undefined;
  toolResultsMap?: Map<string, Message>;
  isLast: boolean;
}

export const ToolGroupContent: React.FC<ToolGroupContentProps> = ({
  items,
  message,
  toolResultsMap,
  isLast,
}) => {
  const toolGroupCalls = items.map((tc) => ({
    id: tc.id,
    type: 'function' as const,
    function: { name: tc.name, arguments: tc.arguments },
  }));
  const toolGroupResults = toolGroupCalls.map((call) =>
    toolResultsMap?.get(call.id),
  );

  return (
    <div className="my-2">
      <AgentToolCallGroup
        message={
          message ||
          ({
            id: 'dummy',
            role: 'assistant',
            content: [],
          } as unknown as Message)
        } // dummy message if missing, mostly for ID
        toolGroup={{ calls: toolGroupCalls }}
        toolResults={toolGroupResults}
        isLast={isLast} // flawed if text follows, but acceptable for visibility logic
        visibleCount={999} // Expand by default for interleaved? Or keep default 3? Let's default.
      />
    </div>
  );
};
