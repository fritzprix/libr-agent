import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Button } from '@/components/ui';
import { useAssistantContext } from '@/context/AssistantContext';
import {
  useAgentSessionState,
  useAgentSessionActions,
} from '@/context/AgentSessionContext';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import { cn } from '@/lib/utils';

const logger = getLogger('StartAgentView');

/**
 * Start Agent View (Simple MVP)
 *
 * Minimal UI for creating a new agent session.
 * Displays assistant list and handles session creation.
 *
 * Pattern: Similar to V1's StartChatView but simplified for MVP
 */
export default function StartAgentView() {
  const navigate = useNavigate();
  const { assistants } = useAssistantContext();
  const { isLoading, error } = useAgentSessionState();
  const { createSession } = useAgentSessionActions();
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const handleStart = async () => {
    const assistant = assistants.find((a) => a.id === selectedId);
    if (!assistant) {
      toast.error('Please select an assistant');
      return;
    }

    try {
      const session = await createSession({ assistant });
      logger.info('Agent session created', { sessionId: session.id });
      toast.success('Agent session started');
      // Navigate to session-specific route
      navigate(`/agent/${session.id}`);
    } catch (err) {
      logger.error('Failed to create agent session', err);
      toast.error('Failed to start agent session');
    }
  };

  return (
    <div className="flex flex-col items-center justify-center h-full p-8">
      <h1 className="text-2xl font-bold mb-8">Start Agent Session</h1>

      {/* Assistant List */}
      <div className="w-full max-w-md space-y-4">
        {assistants.map((assistant) => (
          <button
            key={assistant.id}
            onClick={() => setSelectedId(assistant.id || null)}
            className={cn(
              'w-full p-4 text-left border rounded-lg transition-colors',
              selectedId === assistant.id
                ? 'border-primary bg-primary/10'
                : 'border-border hover:border-muted-foreground',
            )}
          >
            <div className="font-semibold">{assistant.name}</div>
            {assistant.systemPrompt && (
              <div className="text-sm text-muted-foreground mt-1 line-clamp-2">
                {assistant.systemPrompt}
              </div>
            )}
          </button>
        ))}
      </div>

      {/* Start Button */}
      <Button
        onClick={handleStart}
        disabled={!selectedId || isLoading}
        className="mt-8 px-8 py-2"
      >
        {isLoading ? 'Starting...' : 'Start Agent Session'}
      </Button>

      {/* Error Display */}
      {error && (
        <div className="mt-4 text-destructive text-sm">Error: {error}</div>
      )}
    </div>
  );
}
