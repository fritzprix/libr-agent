import { useState, useCallback, useEffect, useRef } from 'react';
import { useNavigate, Link, useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
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
  const { t } = useTranslation();
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
          setIsCreating(true);
          logger.info('Auto-starting playbook session', { playbookId });

          if (!toastId) toastId = toast.loading(t('agent.start.startingPlaybook', 'Starting playbook...'));

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
            toast.error(t('agent.start.playbookNotFound', 'Playbook not found'));
            return;
          }

          if (toastId)
            toast.loading(t('agent.start.startingPlaybookNamed', 'Starting playbook: {{name}}', { name: playbook.goal }), {
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
          toast.error(t('agent.start.startPlaybookFailed', 'Failed to start playbook session'));
        } finally {
          processingPlaybookRef.current = false;
          setIsCreating(false);
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
    <main className="h-full w-full flex flex-col items-center overflow-y-auto bg-background/50">
      <div className="w-full max-w-5xl px-8 py-20 flex flex-col gap-16">
        {/* Hero Header */}
        <div className="text-center space-y-4 animate-in fade-in slide-in-from-top-4 duration-700">
          <h1 className="text-4xl font-bold tracking-tight text-foreground sm:text-5xl">
            {t('agent.start.heroTitle', 'What would you like to do today?')}
          </h1>
          <p className="text-muted-foreground text-lg max-w-2xl mx-auto font-sans">
            {t(
              'agent.start.heroSubtitle',
              'Select an assistant to begin a new autonomous session.',
            )}
          </p>
        </div>

        {/* Built-in Assistants */}
        {builtinAssistants.length > 0 && (
          <section
            aria-labelledby="builtin-heading"
            className="animate-in fade-in slide-in-from-bottom-4 duration-700 delay-150"
          >
            <h2
              id="builtin-heading"
              className="text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground/60 mb-6 font-sans ml-1"
            >
              {t('agent.start.builtinAssistants', 'Built-in Assistants')}
            </h2>
            {renderGrid(builtinAssistants)}
          </section>
        )}

        {/* My Assistants */}
        {customAssistants.length > 0 && (
          <section
            aria-labelledby="custom-heading"
            className="animate-in fade-in slide-in-from-bottom-4 duration-700 delay-300"
          >
            <h2
              id="custom-heading"
              className="text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground/60 mb-6 font-sans ml-1"
            >
              {t('agent.start.myAssistants', 'My Assistants')}
            </h2>
            {renderGrid(customAssistants)}
          </section>
        )}

        {/* Empty state */}
        {assistants.length === 0 && (
          <div className="text-center text-muted-foreground py-16">
            <p className="text-sm">{t('agent.start.noAssistants', 'No assistants available yet.')}</p>
            <Link to="/assistants">
              <Button className="mt-4">{t('agent.start.createAssistant', 'Create Assistant')}</Button>
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
                {t('agent.start.manageAssistants', '+ Manage Assistants')}
              </Button>
            </Link>
          </div>
        )}
      </div>
    </main>
  );
}
