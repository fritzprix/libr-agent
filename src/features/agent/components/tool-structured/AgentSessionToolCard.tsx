import React, { useCallback, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import {
  AlertTriangle,
  CheckCircle2,
  CircleDashed,
  Copy,
  ExternalLink,
  FolderOpen,
  Loader2,
  PauseCircle,
  Send,
  Sparkles,
  StopCircle,
  Trash2,
  Timer,
} from 'lucide-react';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import { getLogger } from '@/lib/logger';
import { ExpandableScrollText } from './ExpandableScrollText';
import {
  classifyAgentSessionCard,
  readAgentToolArgString,
  resolveAgentSessionId,
  type AgentSessionCardKind,
  type AgentSessionToolResult,
} from './agent-types';

const logger = getLogger('AgentSessionToolCard');

export interface AgentSessionToolCardProps {
  toolName: string;
  data: AgentSessionToolResult;
  /** Tool-call arguments (task / message) for human context fallback. */
  toolArgs?: Record<string, unknown>;
}

function statusKey(status: string | undefined): string {
  return (status ?? '').toLowerCase();
}

function kindIcon(kind: AgentSessionCardKind, status?: string) {
  if (kind === 'needs_attention' && statusKey(status) === 'paused') {
    return PauseCircle;
  }
  switch (kind) {
    case 'spawned':
      return Sparkles;
    case 'instruction_sent':
      return Send;
    case 'in_progress':
      return Loader2;
    case 'wait_timeout':
      return Timer;
    case 'finished':
      return CheckCircle2;
    case 'needs_attention':
      return AlertTriangle;
    case 'stopped':
      return StopCircle;
    case 'deleted':
      return Trash2;
    default:
      return CircleDashed;
  }
}

function kindBadgeClass(kind: AgentSessionCardKind): string {
  switch (kind) {
    case 'finished':
      return 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-300';
    case 'needs_attention':
    case 'stopped':
      return 'bg-destructive/15 text-destructive';
    case 'wait_timeout':
    case 'in_progress':
      return 'bg-warning/15 text-warning-foreground';
    case 'deleted':
      return 'bg-muted text-muted-foreground';
    default:
      return 'bg-muted text-foreground';
  }
}

/**
 * Human-facing structured view for agent__* session tools.
 * CTAs are user actions only (open child / copy) — never agent wait/message next-steps.
 */
export const AgentSessionToolCard: React.FC<AgentSessionToolCardProps> = ({
  toolName,
  data,
  toolArgs,
}) => {
  const { t } = useTranslation('common');
  const navigate = useNavigate();
  const [copying, setCopying] = useState(false);

  const kind = classifyAgentSessionCard(toolName, data);
  const sessionId = resolveAgentSessionId(data);

  const mission =
    data.task?.trim() || readAgentToolArgString(toolArgs, 'task') || undefined;
  const instruction =
    data.instruction?.trim() ||
    readAgentToolArgString(toolArgs, 'message') ||
    undefined;

  const resultText = data.result?.trim() || '';
  const recentSummaries =
    data.latestMessages
      ?.map((m) => m.summary.trim())
      .filter((s) => s.length > 0)
      .slice(0, 3) ?? [];

  const title = (() => {
    if (!kind) {
      return t('agent.toolStructured.agentCard.fallback', 'Sub-agent');
    }
    switch (kind) {
      case 'spawned':
        return t('agent.toolStructured.agentCard.spawned', 'Started');
      case 'instruction_sent':
        return t('agent.toolStructured.agentCard.messageSent', 'Messaged');
      case 'in_progress':
        return t('agent.toolStructured.agentCard.inProgress', 'Working');
      case 'wait_timeout':
        return t('agent.toolStructured.agentCard.waitTimeout', 'Still working');
      case 'finished':
        return t('agent.toolStructured.agentCard.finished', 'Finished');
      case 'needs_attention':
        if (statusKey(data.status) === 'paused') {
          return t('agent.toolStructured.agentCard.paused', 'Paused');
        }
        if (statusKey(data.status) === 'terminated') {
          return t(
            'agent.toolStructured.agentCard.terminated',
            'Stopped early',
          );
        }
        return t('agent.toolStructured.agentCard.failed', 'Needs attention');
      case 'stopped':
        return data.stopped === false
          ? t('agent.toolStructured.agentCard.stopNoop', 'Already stopped')
          : t('agent.toolStructured.agentCard.stopped', 'Stopped');
      case 'deleted':
        return t('agent.toolStructured.agentCard.deleted', 'Removed');
      default:
        return t('agent.toolStructured.agentCard.fallback', 'Sub-agent');
    }
  })();

  // Hide raw transport statuses (processed/accepted/pending) — not meaningful to laypeople.
  const statusLabel = (() => {
    if (!kind) return null;
    if (kind === 'instruction_sent' || kind === 'spawned') return null;
    const raw = (data.status ?? data.responseStatus ?? '').toLowerCase();
    if (!raw) return null;
    if (['pending', 'accepted', 'processed', 'started', 'noop'].includes(raw)) {
      return null;
    }
    return data.status ?? data.responseStatus ?? null;
  })();

  const assistantName = data.assistantName?.trim() || undefined;
  // Prefer persona name; fall back to session id only when name is unknown.
  const primaryIdentity = assistantName ?? sessionId ?? undefined;

  const handleOpen = useCallback(() => {
    if (!sessionId) return;
    navigate(`/agent/${sessionId}`);
  }, [navigate, sessionId]);

  const handleCopyResult = useCallback(async () => {
    if (!resultText || copying) return;
    setCopying(true);
    try {
      await navigator.clipboard.writeText(resultText);
      toast.success(
        t('agent.toolStructured.agentCard.copied', 'Result copied'),
      );
    } catch (error) {
      logger.error('Failed to copy child result', error);
      toast.error(
        t('agent.toolStructured.agentCard.copyError', 'Failed to copy result'),
      );
    } finally {
      setCopying(false);
    }
  }, [copying, resultText, t]);

  if (!kind) return null;

  const Icon = kindIcon(kind, data.status);
  const canOpen = Boolean(sessionId) && kind !== 'deleted';
  const showResultPanel =
    (kind === 'finished' || kind === 'needs_attention') &&
    resultText.length > 0;
  const collapsedBodyText =
    kind === 'instruction_sent'
      ? instruction
      : kind === 'spawned'
        ? mission
        : undefined;
  const showWorkspaceMeta = kind === 'spawned';

  return (
    <div
      data-testid="tool-structured-agent-session"
      data-card-kind={kind}
      className="space-y-2.5 text-sm"
    >
      <div className="flex items-start gap-2">
        <Icon
          className={cn(
            'mt-0.5 h-4 w-4 shrink-0 text-muted-foreground',
            kind === 'in_progress' && 'animate-spin',
          )}
        />
        <div className="min-w-0 flex-1 space-y-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="font-medium">{title}</span>
            {statusLabel ? (
              <span
                className={cn(
                  'rounded px-1.5 py-0.5 text-xs font-medium capitalize',
                  kindBadgeClass(kind),
                )}
              >
                {statusLabel}
              </span>
            ) : null}
            {typeof data.turnCount === 'number' ? (
              <span className="text-xs text-muted-foreground">
                {t('agent.toolStructured.agentCard.turns', '{{count}} turns', {
                  count: data.turnCount,
                })}
              </span>
            ) : null}
          </div>

          {primaryIdentity ? (
            <p
              className={cn(
                'truncate',
                assistantName
                  ? 'text-sm font-medium text-foreground'
                  : 'font-mono text-xs text-muted-foreground',
              )}
              title={primaryIdentity}
            >
              {primaryIdentity}
            </p>
          ) : null}

          {showWorkspaceMeta ? (
            <div className="flex flex-wrap items-center gap-1.5 text-xs text-muted-foreground">
              <FolderOpen className="h-3.5 w-3.5 shrink-0" />
              {data.workspaceOverride ? (
                <span
                  className="truncate"
                  title={data.workspacePath}
                  data-testid="agent-session-workspace-override"
                >
                  {t(
                    'agent.toolStructured.agentCard.workspaceOverride',
                    'Workspace override',
                  )}
                  {data.workspacePath ? `: ${data.workspacePath}` : ''}
                </span>
              ) : (
                <span data-testid="agent-session-workspace-default">
                  {data.workspacePath
                    ? t(
                        'agent.toolStructured.agentCard.workspaceInherited',
                        'Shared workspace (org default)',
                      )
                    : t(
                        'agent.toolStructured.agentCard.workspaceIsolated',
                        'Isolated workspace (default)',
                      )}
                </span>
              )}
            </div>
          ) : null}

          {kind === 'wait_timeout' ? (
            <p className="text-xs text-muted-foreground">
              {t(
                'agent.toolStructured.agentCard.waitTimeoutHint',
                'The parent agent stopped waiting; the child may still be working.',
              )}
              {typeof data.timeoutSeconds === 'number'
                ? ` (${data.timeoutSeconds}s)`
                : null}
            </p>
          ) : null}

          {kind === 'stopped' && data.stopped !== false ? (
            <p className="text-xs text-muted-foreground">
              {t(
                'agent.toolStructured.agentCard.stoppedHint',
                'Forced stop by the parent agent — work may be incomplete.',
              )}
            </p>
          ) : null}

          {kind === 'deleted' ? (
            <p className="text-xs text-muted-foreground">
              {typeof data.descendantCount === 'number' &&
              data.descendantCount > 0
                ? t(
                    'agent.toolStructured.agentCard.deletedCascade',
                    'Removed with {{count}} descendant session(s).',
                    { count: data.descendantCount },
                  )
                : t(
                    'agent.toolStructured.agentCard.deletedHint',
                    'Session data removed — no longer available.',
                  )}
            </p>
          ) : null}

          {kind === 'in_progress' && !resultText ? (
            <p className="text-xs text-muted-foreground">
              {t(
                'agent.toolStructured.agentCard.noResultYet',
                'No final answer yet.',
              )}
            </p>
          ) : null}
        </div>
      </div>

      {collapsedBodyText ? (
        <div className="space-y-1">
          <div className="text-[10px] uppercase tracking-wide text-muted-foreground">
            {kind === 'spawned'
              ? t('agent.toolStructured.agentCard.task', 'Task')
              : t('agent.toolStructured.agentCard.message', 'Message')}
          </div>
          <ExpandableScrollText
            text={collapsedBodyText}
            showLabel={t(
              'agent.toolStructured.agentCard.showMore',
              'Show more',
            )}
            hideLabel={t(
              'agent.toolStructured.agentCard.showLess',
              'Show less',
            )}
            data-testid="agent-session-instruction-text"
          />
        </div>
      ) : null}

      {showResultPanel ? (
        <div className="space-y-1">
          <div className="text-[10px] uppercase tracking-wide text-muted-foreground">
            {kind === 'finished'
              ? t('agent.toolStructured.agentCard.result', 'Result')
              : t('agent.toolStructured.agentCard.lastOutput', 'Last output')}
          </div>
          <ExpandableScrollText
            text={resultText}
            data-testid="agent-session-result-text"
          />
        </div>
      ) : null}

      {kind === 'wait_timeout' && recentSummaries.length > 0 ? (
        <div className="space-y-1">
          <div className="text-[10px] uppercase tracking-wide text-muted-foreground">
            {t(
              'agent.toolStructured.agentCard.recentActivity',
              'Recent activity',
            )}
          </div>
          <ul className="max-h-32 space-y-1 overflow-y-auto rounded border bg-muted/30 px-2.5 py-2 text-xs text-muted-foreground">
            {recentSummaries.map((summary, index) => (
              <li key={index} className="truncate">
                · {summary}
              </li>
            ))}
          </ul>
        </div>
      ) : null}

      {(canOpen || (kind === 'finished' && resultText)) && (
        <div className="flex flex-wrap gap-2">
          {canOpen ? (
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={handleOpen}
            >
              <ExternalLink className="mr-1.5 h-3.5 w-3.5" />
              {t('agent.toolStructured.agentCard.openChild', 'Open session')}
            </Button>
          ) : null}
          {kind === 'finished' && resultText ? (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              disabled={copying}
              onClick={() => {
                void handleCopyResult();
              }}
            >
              <Copy className="mr-1.5 h-3.5 w-3.5" />
              {t('agent.toolStructured.agentCard.copyResult', 'Copy result')}
            </Button>
          ) : null}
        </div>
      )}
    </div>
  );
};
