import React, { memo, useMemo } from 'react';
import type { Message } from '@/models/chat';
import type { MCPToolCallContent } from '@/lib/mcp';
import { AgentToolCallGroup } from '../../AgentToolCallGroup';
import { useSettings } from '@/hooks/use-settings';

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
  const {
    value: { toolCallGroupVisibleCount },
  } = useSettings();
  const toolGroupCalls = useMemo(
    () =>
      groupBlock.items.map((tc) => ({
        id: tc.id,
        type: 'function' as const,
        function: { name: tc.name, arguments: tc.arguments },
      })),
    [groupBlock.items],
  );

  const toolGroupResults = useMemo(() => {
    const idUsageCount = new Map<string, number>();
    return toolGroupCalls.map((call) => {
      const count = idUsageCount.get(call.id) || 0;
      idUsageCount.set(call.id, count + 1);
      
      const key = count === 0 ? call.id : `${call.id}_dup${count}`;
      return toolResultsMap?.get(key);
    });
  }, [toolGroupCalls, toolResultsMap]);

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
      visibleCount={toolCallGroupVisibleCount}
    />
  );
};

export const AgentToolGroupBlock = memo(AgentToolGroupBlockImpl);
