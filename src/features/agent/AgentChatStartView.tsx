import { useState, useCallback, useEffect, useMemo, useRef } from 'react';
import { useNavigate, Link, useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { AlertCircle, Settings, Sparkles, X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { useAgentSessionListActions } from '@/context/AgentSessionListContext';
import { AssistantSelectionCard } from './components/AssistantSelectionCard';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import { getPlaybook } from '@/lib/backend/playbooks';
import { getAssistant, type AssistantSummary } from '@/lib/backend/assistants';
import { useAssistantSummaries } from './hooks/useAssistantSummaries';
import { useSettings } from '@/hooks/use-settings';
import { listConfiguredProviderGroups } from '@/lib/ai-service/configured-providers';
import { MorningBriefingWalkthroughDialog } from '@/features/recipes';

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
  const { value: settings } = useSettings();
  const { assistants, loading, error } = useAssistantSummaries();
  const { createSession } = useAgentSessionListActions();
  const [isCreating, setIsCreating] = useState(false);
  const [startingAssistantId, setStartingAssistantId] = useState<string | null>(
    null,
  );
  const [walkthroughOpen, setWalkthroughOpen] = useState(false);
  const [searchParams] = useSearchParams();
  const processingPlaybookRef = useRef(false);
  const playbookId = searchParams.get('playbookId');

  const [isRecipeDismissed, setIsRecipeDismissed] = useState(() => {
    if (typeof window === 'undefined') return false;
    try {
      return (
        localStorage.getItem('libragent:morning-briefing:completed') ===
          'true' ||
        localStorage.getItem('libragent:morning-briefing:dismissed') === 'true'
      );
    } catch {
      return false;
    }
  });

  const isRecipeVisible = useMemo(() => {
    if (isRecipeDismissed) return false;
    if (typeof window === 'undefined') return true;
    try {
      return (
        localStorage.getItem('libragent:morning-briefing:completed') !==
          'true' &&
        localStorage.getItem('libragent:morning-briefing:dismissed') !== 'true'
      );
    } catch {
      return true;
    }
  }, [isRecipeDismissed, walkthroughOpen]);

  const handleDismissRecipe = () => {
    try {
      localStorage.setItem('libragent:morning-briefing:dismissed', 'true');
    } catch (e) {
      logger.warn('Failed to set recipe dismissal flag in localStorage', e);
    }
    setIsRecipeDismissed(true);
  };

  const hasConfiguredProviders = useMemo(
    () => listConfiguredProviderGroups(settings).length > 0,
    [settings],
  );

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
          toast.loading(
            t('agent.start.startingPlaybook', { goal: playbook.goal }),
            {
              id: toastId,
            },
          );

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
    (assistantId: string) => {
      setStartingAssistantId(assistantId);
      navigate(`/agent/draft?assistantId=${assistantId}`);
    },
    [navigate],
  );

  const handleStartSelection = useCallback(
    (assistantId: string) => {
      setIsCreating(true);
      handleAssistantSelect(assistantId);
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

        {/* Onboarding Banner */}
        {!hasConfiguredProviders && (
          <div
            data-testid="onboarding-banner"
            className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 rounded-xl border border-amber-500/30 bg-amber-500/10 p-5 text-card-foreground shadow-sm dark:border-amber-400/30 dark:bg-amber-400/10 animate-in fade-in slide-in-from-top-2 duration-500"
          >
            <div className="flex items-start gap-3">
              <AlertCircle className="h-5 w-5 shrink-0 text-amber-500 dark:text-amber-400 mt-0.5" />
              <div className="space-y-1">
                <h3 className="text-sm font-semibold text-foreground">
                  {t('agent.start.onboardingBannerTitle', {
                    defaultValue: 'AI Model Configuration Required',
                  })}
                </h3>
                <p className="text-xs text-muted-foreground">
                  {t('agent.start.onboardingBannerDesc', {
                    defaultValue:
                      'To start using agent sessions, please configure an AI model provider (OpenAI, Anthropic, Ollama, etc.) first.',
                  })}
                </p>
              </div>
            </div>
            <Button
              onClick={() => navigate('/settings')}
              size="sm"
              className="shrink-0 gap-1.5 self-end sm:self-center"
            >
              <Settings className="h-4 w-4" />
              {t('agent.start.goToSettings', {
                defaultValue: 'Configure AI Model',
              })}
            </Button>
          </div>
        )}

        {/* Featured Recipe Card */}
        {isRecipeVisible && (
          <div
            data-testid="featured-recipe-card"
            className="relative flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 rounded-xl border border-primary/20 bg-primary/5 p-5 text-card-foreground shadow-sm animate-in fade-in slide-in-from-top-2 duration-500"
          >
            <Button
              variant="ghost"
              size="icon"
              data-testid="dismiss-recipe-button"
              className="absolute top-2 right-2 h-7 w-7 text-muted-foreground hover:text-foreground"
              aria-label={t('agent.start.dismissRecipe', {
                defaultValue: '추천 레시피 닫기',
              })}
              onClick={handleDismissRecipe}
            >
              <X className="h-4 w-4" />
            </Button>
            <div className="flex items-start gap-3 pr-8 sm:pr-0">
              <div className="rounded-lg bg-primary/10 p-2.5 text-primary shrink-0 mt-0.5">
                <Sparkles className="h-5 w-5" />
              </div>
              <div className="space-y-1">
                <div className="flex items-center gap-2">
                  <h3 className="text-sm font-semibold text-foreground">
                    {t(
                      'agent.start.featuredRecipeTitle',
                      '🌅 5분 만에 만드는 나만의 모닝 브리핑 비서',
                    )}
                  </h3>
                  <Badge
                    variant="default"
                    className="text-[10px] px-1.5 py-0 h-4 bg-primary/80"
                  >
                    YOLO
                  </Badge>
                </div>
                <p className="text-xs text-muted-foreground">
                  {t(
                    'agent.start.featuredRecipeDesc',
                    'Hacker News의 테크 트렌드와 Yahoo Finance 실시간 지표를 결합해 매일 아침 브리핑합니다.',
                  )}
                </p>
              </div>
            </div>
            <Button
              onClick={() => setWalkthroughOpen(true)}
              size="sm"
              className="shrink-0 gap-1.5 self-end sm:self-center"
            >
              {t('agent.start.startWalkthrough', '가이드 워크스루 시작')}
            </Button>
          </div>
        )}

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
            <p className="text-sm">{t('agent.start.assistantsLoadFailed')}</p>
          </div>
        )}

        {!loading && !error && assistants.length === 0 && (
          <div className="text-center text-muted-foreground py-16">
            <p className="text-sm">{t('agent.start.noAssistantsAvailable')}</p>
            <Link to="/assistants">
              <Button className="mt-4">
                {t('agent.start.createAssistant')}
              </Button>
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

      <MorningBriefingWalkthroughDialog
        open={walkthroughOpen}
        onOpenChange={setWalkthroughOpen}
      />
    </main>
  );
}
