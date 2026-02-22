import { useState, useCallback, useEffect, useRef } from 'react';
import { useNavigate, Link, useSearchParams } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import { useAssistantContext } from '@/context/AssistantContext';
import { useAgentSessionListActions } from '@/context/AgentSessionListContext';
import { AssistantSelectionCard } from './components/AssistantSelectionCard';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import type { Assistant } from '@/models/chat';
import { getPlaybook } from '@/lib/backend/playbooks';

const logger = getLogger('AgentChatStartView');

/**
 * Agent Chat Start View – Hub Layout
 *
 * Clean, centered assistant picker. History has moved to the sidebar
 * and the dedicated History page. No session history panel here.
 *
 * Assistants are grouped into:
 *  - Built-in Assistants  (deletionProtected === true)
 *  - My Assistants        (deletionProtected === false), shown only if any exist.
 */
export default function AgentChatStartView() {
  const navigate = useNavigate();
  const { assistants } = useAssistantContext();
  const { createSession } = useAgentSessionListActions();
  const [isCreating, setIsCreating] = useState(false);
  const [startingAssistantId, setStartingAssistantId] = useState<string | null>(
    null,
  );
  const [searchParams] = useSearchParams();
  const processingPlaybookRef = useRef(false);

  // Handle Playbook Auto-Start
  useEffect(() => {
    const playbookId = searchParams.get('playbookId');
    if (playbookId && !processingPlaybookRef.current && assistants.length > 0) {
      let toastId: string | number | undefined;

      const initPlaybookSession = async () => {
        try {
          processingPlaybookRef.current = true;
          logger.info('Auto-starting playbook session', { playbookId });

          if (!toastId) toastId = toast.loading('Starting playbook...');

          const allAssistants = assistants;
          let playbook = null;
          let targetAssistant = null;

          for (const assistant of allAssistants) {
            try {
              playbook = await getPlaybook(playbookId, assistant.id);
              if (playbook) {
                targetAssistant = assistant;
                break;
              }
            } catch {
              continue;
            }
          }

          if (!playbook || !targetAssistant) {
            if (toastId) toast.dismiss(toastId);
            toast.error('Playbook not found');
            return;
          }

          if (toastId)
            toast.loading(`Starting playbook: ${playbook.goal}`, {
              id: toastId,
            });

          const session = await createSession({
            assistant: targetAssistant,
            name: playbook.goal,
          });
          if (toastId) toast.dismiss(toastId);

          navigate(`/agent/${session.id}?playbookId=${playbookId}`);
        } catch (error) {
          if (toastId) toast.dismiss(toastId);
          logger.error('Failed to start playbook session', error);
          toast.error('Failed to start playbook session');
        } finally {
          processingPlaybookRef.current = false;
        }
      };

      initPlaybookSession();
    }
  }, [searchParams, assistants, createSession, navigate]);

  const handleAssistantSelect = useCallback(
    (assistant: Assistant) => {
      setStartingAssistantId(assistant.id);
      navigate(`/agent/draft?assistantId=${assistant.id}`);
    },
    [navigate],
  );

  // Split assistants into built-in vs user-created
  const builtinAssistants = assistants.filter((a) => a.deletionProtected);
  const customAssistants = assistants.filter((a) => !a.deletionProtected);

  const renderGrid = (list: Assistant[]) => (
    <ul className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3 list-none">
      {list.map((assistant) => {
        const isThisStarting = startingAssistantId === assistant.id;
        return (
          <li key={assistant.id}>
            <AssistantSelectionCard
              assistant={assistant}
              isStarting={isThisStarting}
              disabled={isCreating}
              onSelect={(a) => {
                setIsCreating(true);
                handleAssistantSelect(a);
              }}
            />
          </li>
        );
      })}
    </ul>
  );

  return (
    <main className="h-full w-full flex flex-col items-center overflow-y-auto bg-background">
      <div className="w-full max-w-4xl px-6 py-16 flex flex-col gap-12">
        {/* Hero Header */}
        <div className="text-center space-y-3">
          <h1 className="text-4xl font-semibold tracking-tight text-foreground">
            What would you like to do today?
          </h1>
          <p className="text-muted-foreground text-base">
            Select an assistant to begin a new session.
          </p>
        </div>

        {/* Built-in Assistants */}
        {builtinAssistants.length > 0 && (
          <section aria-labelledby="builtin-heading">
            <h2
              id="builtin-heading"
              className="text-xs font-semibold uppercase tracking-widest text-muted-foreground mb-3"
            >
              Built-in Assistants
            </h2>
            {renderGrid(builtinAssistants)}
          </section>
        )}

        {/* My Assistants – only rendered when user has created at least one */}
        {customAssistants.length > 0 && (
          <section aria-labelledby="custom-heading">
            <h2
              id="custom-heading"
              className="text-xs font-semibold uppercase tracking-widest text-muted-foreground mb-3"
            >
              My Assistants
            </h2>
            {renderGrid(customAssistants)}
          </section>
        )}

        {/* Empty state */}
        {assistants.length === 0 && (
          <div className="text-center text-muted-foreground py-16">
            <p className="text-sm">No assistants available yet.</p>
            <Link to="/assistants">
              <Button className="mt-4">Create Assistant</Button>
            </Link>
          </div>
        )}

        {/* Footer action */}
        {assistants.length > 0 && (
          <div className="flex justify-center pt-2">
            <Link to="/assistants">
              <Button
                variant="outline"
                disabled={isCreating}
                size="sm"
                className="text-xs"
              >
                + Manage Assistants
              </Button>
            </Link>
          </div>
        )}
      </div>
    </main>
  );
}
