import { Button } from '@/components/ui';
import { useAssistantContext } from '@/context/AssistantContext';
import { useSessionContext } from '@/context/SessionContext';
import { useMCPServer } from '@/hooks/use-mcp-server';
import { Assistant } from '@/models/chat';
import { getLogger } from '@/lib/logger';
import { useCallback, useState } from 'react';
import { toast } from 'sonner';
import { Link } from 'react-router';

const logger = getLogger('StartChatView');

export default function StartChatView() {
  const { assistants, setCurrentAssistant } = useAssistantContext();
  const { start } = useSessionContext();
  const { connectServersFromAssistant } = useMCPServer();
  const [isStarting, setIsStarting] = useState(false);
  const [startingAssistantId, setStartingAssistantId] = useState<string | null>(
    null,
  );

  const handleAssistantSelect = useCallback(
    async (assistant: Assistant) => {
      if (isStarting) return; // 중복 클릭 방지

      try {
        setIsStarting(true);
        setStartingAssistantId(assistant.id || null);
        logger.info('Starting chat with assistant', {
          assistantId: assistant.id,
          assistantName: assistant.name,
        });

        // 1. Assistant 설정
        setCurrentAssistant(assistant);

        // 2. MCP 서버 연결 완료까지 대기
        logger.debug('Connecting MCP servers for assistant', {
          mcpServerIds: assistant.mcpServerIds,
        });
        await connectServersFromAssistant(assistant);
        logger.info('MCP servers connected successfully', {
          assistantId: assistant.id,
        });

        // 3. 이제 안전하게 세션 시작
        await start([assistant]);
        logger.info('Chat session started successfully', {
          assistantId: assistant.id,
        });
      } catch (error) {
        logger.error('Failed to start chat with assistant', {
          assistantId: assistant.id,
          error: error instanceof Error ? error.message : String(error),
        });
        toast.error('챗 세션 시작에 실패했습니다');
      } finally {
        setIsStarting(false);
        setStartingAssistantId(null);
      }
    },
    [start, setCurrentAssistant, connectServersFromAssistant, isStarting],
  );

  return (
    <div className="h-full w-full flex flex-col items-center justify-center font-mono p-4 space-y-2">
      <h2 className="text-2xl font-bold mb-6">
        Select an Assistant to Start a Chat
      </h2>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 w-full max-w-4xl">
        {assistants.map((assistant) => {
          const isThisStarting = startingAssistantId === assistant.id;
          return (
            <div
              key={assistant.id}
              className={`border rounded-lg p-4 cursor-pointer transition-colors ${
                isStarting && !isThisStarting
                  ? 'opacity-50 cursor-not-allowed'
                  : 'hover:bg-muted/50'
              }`}
              onClick={() => !isStarting && handleAssistantSelect(assistant)}
            >
              <h3 className="text-lg font-semibold flex items-center gap-2">
                {assistant.name}
                {isThisStarting && (
                  <span className="text-sm text-blue-600">Starting...</span>
                )}
              </h3>
              <p className="text-sm mt-2 line-clamp-3">
                {assistant.systemPrompt}
              </p>
            </div>
          );
        })}
      </div>
      <Link to={'/assistants'}>
        <Button disabled={isStarting}>Manage Assistants</Button>
      </Link>
    </div>
  );
}
