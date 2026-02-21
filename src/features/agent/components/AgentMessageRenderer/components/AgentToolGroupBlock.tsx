import React, { memo, useMemo } from 'react';
import type { Message } from '@/models/chat';
import type { MCPToolCallContent } from '@/lib/mcp';
import { AgentToolCallGroup } from '../../AgentToolCallGroup';

interface AgentToolGroupBlockProps {
  message: Message;
  groupBlock: {
    type: 'tool_group_block';
    items: MCPToolCallContent[];
  };
  toolResultsMap?: Map<string, Message>;
  isLast: boolean;
}

const AgentToolGroupBlockImpl: React.FC<AgentToolGroupBlockProps> = ({
  message,
  groupBlock,
  toolResultsMap,
  isLast,
}) => {
  const toolGroupCalls = useMemo(
    () =>
      groupBlock.items.map((tc) => ({
        id: tc.id,
        type: 'function' as const,
        function: { name: tc.name, arguments: tc.arguments },
      })),
    [groupBlock.items],
  );

  const toolGroupResults = useMemo(
    () => toolGroupCalls.map((call) => toolResultsMap?.get(call.id)),
    [toolGroupCalls, toolResultsMap],
  );

  const toolGroup = useMemo(
    () => ({ calls: toolGroupCalls }),
    [toolGroupCalls],
  );

  return (
    <AgentToolCallGroup
      message={message}
      toolGroup={toolGroup}
      toolResults={toolGroupResults}
      isLast={isLast}
      visibleCount={999}
    />
  );
};

export const AgentToolGroupBlock = memo(AgentToolGroupBlockImpl);
