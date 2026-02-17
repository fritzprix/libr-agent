import React, { memo, useMemo } from 'react';
import { AgentToolCallGroup } from '../../AgentToolCallGroup';
import type { MCPToolCallContent } from '@/lib/mcp';
import type { Message } from '@/models/chat';
import { useStableArray } from '@/hooks/useStableArray';

export interface ToolGroupBlockProps {
  groupBlock: { type: 'tool_group_block'; items: MCPToolCallContent[] };
  message?: Message;
  toolResultsMap?: Map<string, Message>;
  isLast: boolean;
}

const ToolGroupBlockImpl: React.FC<ToolGroupBlockProps> = ({
  groupBlock,
  message,
  toolResultsMap,
  isLast,
}) => {
  // Stabilize items array based on content references
  const stableItems = useStableArray(groupBlock.items);

  const toolGroupCalls = useMemo(() => {
    return stableItems.map((tc) => ({
      id: tc.id,
      type: 'function' as const,
      function: { name: tc.name, arguments: tc.arguments },
    }));
  }, [stableItems]);

  const toolGroupResults = useMemo(() => {
    return toolGroupCalls.map((call) => toolResultsMap?.get(call.id));
  }, [toolGroupCalls, toolResultsMap]);

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
        }
        toolGroup={{ calls: toolGroupCalls }}
        toolResults={toolGroupResults}
        isLast={isLast}
        visibleCount={999}
      />
    </div>
  );
};

export const ToolGroupBlock = memo(ToolGroupBlockImpl);
