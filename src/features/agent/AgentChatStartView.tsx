import { useState, useCallback, useEffect, useMemo, useRef } from 'react';
import { useNavigate, Link, useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { useAgentSessionListActions } from '@/context/AgentSessionListContext';
import { AssistantSelectionCard } from './components/AssistantSelectionCard';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import { getPlaybook } from '@/lib/backend/playbooks';
import { getAssistant, type AssistantSummary } from '@/lib/backend/assistants';
import { useAssistantSummaries } from './hooks/useAssistantSummaries';

const logger = getLogger('AgentChatStartView');

type PlaybookMatch = {
  assistant: AssistantSummary;
  playbook: NonNullable<Awaited<ReturnType<typeof getPlaybook>>>;
};

async function findPlaybookMatch(
  playbookId: string,
  assistants: AssistantSummary[],
): Promise<PlaybookMatch | null> {
  const playbookChecks = assistants.map(async (assistant) => {
    try {
      const playbook = await getPlaybook(playbookId, assistant.id);
      return playbook ? { assistant, playbook } : null;
    } catch {
      return null;
    }
  });

  const matches = await Promise.all(playbookChecks);
  return (
    matches.find((match): match is PlaybookMatch => match !== null) ?? null
  );
}

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
  const { assistants, loading, error } = useAssistantSummaries();
  const { createSession } = useAgentSessionListActions();
  const [isCreating, setIsCreating] = useState(false);
  const [startingAssistantId, setStartingAssistantId] = useState<string | null>(
    null,
  );
  const [searchParams] = useSearchParams();
  const processingPlaybookRef = useRef(false);
  const playbookId = searchParams.get('playbookId');

  // Handle Playbook Auto-Start
  useEffect(() => {
    if (
      !playbookId ||
      processingPlaybookRef.current ||
      assistants.length === 0
    ) {
      return;
    }

    let toastId: string | number | undefined;

    const initPlaybookSession = async () => {
      try {
        processingPlaybookRef.current = true;
        setIsCreating(true);
        logger.info('Auto-starting playbook session', { playbookId });

        if (!toastId) toastId = toast.loading('Starting playbook...');

        const match = await findPlaybookMatch(playbookId, assistants);

        if (!match) {
          if (toastId) toast.dismiss(toastId);
          toast.error(t('agent.start.playbookNotFound'));
          return;
        }

        const { assistant: targetAssistant, playbook } = match;
        const fullAssistant = await getAssistant(targetAssistant.id);

        if (!fullAssistant) {
          if (toastId) toast.dismiss(toastId);
          toast.error(t('agent.start.assistantNotFound'));
          return;
        }

        if (toastId)
          toast.loading(t('agent.start.startingPlaybook', { goal: playbook.goal }), {
            id: toastId,
          });

        const session = await createSession({
          assistant: fullAssistant,
          name: playbook.goal,
        });
        if (toastId) toast.dismiss(toastId);

        navigate(`/agent/${session.id}?playbookId=${playbookId}`);
      } catch (error) {
        if (toastId) toast.dismiss(toastId);
        logger.error('Failed to start playbook session', error);
        toast.error(t('agent.start.failedToStartPlaybookSession'));
      } finally {
        processingPlaybookRef.current = false;
        setIsCreating(false);
      }
    };

    void initPlaybookSession();
  }, [assistants, createSession, navigate, playbookId]);

  const handleAssistantSelect = useCallback(
    (assistant: AssistantSummary) => {
      setStartingAssistantId(assistant.id);
      navigate(`/agent/draft?assistantId=${assistant.id}`);
    },
    [navigate],
  );

  const handleStartSelection = useCallback(
    (assistant: AssistantSummary) => {
      setIsCreating(true);
      handleAssistantSelect(assistant);
    },
    [handleAssistantSelect],
  );

  const { builtinAssistants, customAssistants } = useMemo(() => {
    const builtin: AssistantSummary[] = [];
    const custom: AssistantSummary[] = [];

    for (const assistant of assistants) {
      if (assistant.deletionProtected) {
        builtin.push(assistant);
      } else {
        custom.push(assistant);
      }
    }

    return {
      builtinAssistants: builtin,
      customAssistants: custom,
    };
  }, [assistants]);

  const renderGrid = (list: AssistantSummary[]) => (
    <ul className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3 list-none">
      {list.map((assistant) => {
        const isThisStarting = startingAssistantId === assistant.id;
        return (
          <li key={assistant.id}>
            <AssistantSelectionCard
              assistant={assistant}
              isStarting={isThisStarting}
              disabled={isCreating}
              onSelect={handleStartSelection}
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
            {t('agent.start.heroTitle')}
          </h1>
          <p className="text-muted-foreground text-lg max-w-2xl mx-auto font-sans">
            {t('agent.start.heroSubtitle')}
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
              {t('agent.start.builtinAssistants')}
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
              {t('agent.start.myAssistants')}
            </h2>
            {renderGrid(customAssistants)}
          </section>
        )}

        {/* Empty state */}
        {loading && (
          <div className="text-center text-muted-foreground py-16">
            <p className="text-sm">{t('common.loading')}</p>
          </div>
        )}

        {!loading && error && (
          <div className="text-center text-muted-foreground py-16">
            <p className="text-sm">
              {t('agent.start.assistantsLoadFailed')}
            </p>
          </div>
        )}

        {!loading && !error && assistants.length === 0 && (
          <div className="text-center text-muted-foreground py-16">
            <p className="text-sm">{t('agent.start.noAssistantsAvailable')}</p>
            <Link to="/assistants">
              <Button className="mt-4">{t('agent.start.createAssistant')}</Button>
            </Link>
          </div>
        )}

        {/* Footer action */}
        {!loading && !error && assistants.length > 0 && (
          <div className="flex justify-center pt-2">
            <Link to="/assistants">
              <Button
                variant="outline"
                disabled={isCreating}
                size="sm"
                className="text-xs"
              >
                {t('agent.start.manageAssistants')}
              </Button>
            </Link>
          </div>
        )}
      </div>
    </main>
  );
}
