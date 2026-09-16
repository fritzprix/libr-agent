import React, { useCallback, useMemo } from 'react';
import {
  CheckCircle2,
  AlertTriangle,
  XCircle,
  Package,
  ClipboardCheck,
  ShieldCheck,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { toast } from 'sonner';
import {
  openExternalUrl,
  openPathWithDefaultApp,
  openWorkspaceFileWithDefaultApp,
} from '@/lib/backend';
import { getLogger } from '@/lib/logger';
import { useOptionalAgentFilePreview } from '@/context/AgentFilePreviewContext';
import { useOptionalAgentSessionState } from '@/context/AgentSessionContext';
import type { ReportResultData } from './types';
import { DeliverableFileActions } from './DeliverableFileActions';
import {
  classifyReportResultLink,
  displayNameForWorkspacePath,
} from './reportResultLinks';
import { canOpenInAppPreview } from '../workspace-panel/filePreview';
import { cn } from '@/lib/utils';

const logger = getLogger('ReportResultCard');

export interface ReportResultCardProps {
  data: ReportResultData;
  sessionId?: string;
}

export const ReportResultCard: React.FC<ReportResultCardProps> = ({
  data,
  sessionId: propSessionId,
}) => {
  const { t } = useTranslation('common');
  const sessionContext = useOptionalAgentSessionState();
  const filePreview = useOptionalAgentFilePreview();
  const activeSessionId = propSessionId || sessionContext?.session?.id;

  const statusConfig = (() => {
    switch (data.status) {
      case 'success':
        return {
          icon: CheckCircle2,
          label: t('agent.toolStructured.statusSuccess', 'Completed'),
          badgeClass:
            'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border-emerald-500/20',
          containerBorder: 'border-l-4 border-l-emerald-500',
        };
      case 'partial':
        return {
          icon: AlertTriangle,
          label: t('agent.toolStructured.statusPartial', 'Partial'),
          badgeClass:
            'bg-amber-500/10 text-amber-600 dark:text-amber-400 border-amber-500/20',
          containerBorder: 'border-l-4 border-l-amber-500',
        };
      case 'blocked':
        return {
          icon: XCircle,
          label: t('agent.toolStructured.statusBlocked', 'Blocked'),
          badgeClass:
            'bg-destructive/10 text-destructive border-destructive/20',
          containerBorder: 'border-l-4 border-l-destructive',
        };
    }
  })();

  const StatusIcon = statusConfig.icon;
  const displayTitle =
    data.title ||
    (data.status === 'partial'
      ? t('agent.toolStructured.partialResultTitle', 'Partial result')
      : data.status === 'blocked'
        ? t('agent.toolStructured.blockedTitle', 'Blocked')
        : t('agent.toolStructured.resultTitle', 'Final result'));

  const handleResultLinkClick = useCallback(
    async (event: React.MouseEvent<HTMLAnchorElement>, href: string) => {
      // Never let relative markdown hrefs hit the SPA router.
      event.preventDefault();
      event.stopPropagation();

      const action = classifyReportResultLink(href, data.deliverables);
      try {
        switch (action.kind) {
          case 'external':
            await openExternalUrl(action.url);
            return;
          case 'workspace': {
            if (filePreview && canOpenInAppPreview({ path: action.path })) {
              filePreview.openFilePreview({
                path: action.path,
                name: displayNameForWorkspacePath(action.path),
                sessionId: activeSessionId,
              });
              return;
            }
            if (!activeSessionId) {
              toast.error(
                t(
                  'agent.toolStructured.noSessionForDownload',
                  'No active session found for download',
                ),
              );
              return;
            }
            await openWorkspaceFileWithDefaultApp(action.path, activeSessionId);
            return;
          }
          case 'host':
            await openPathWithDefaultApp(action.absolutePath);
            return;
          case 'blocked':
            toast.error(
              t(
                'agent.toolStructured.unsupportedResultLink',
                'This link cannot be opened from the result panel',
              ),
            );
            return;
        }
      } catch (error) {
        logger.error('Failed to open reportResult markdown link', {
          href,
          error,
        });
        toast.error(
          t('agent.toolStructured.openFileError', 'Failed to open file'),
        );
      }
    },
    [activeSessionId, data.deliverables, filePreview, t],
  );

  const markdownComponents = useMemo(
    () => ({
      a: ({
        href,
        children,
        ...props
      }: React.AnchorHTMLAttributes<HTMLAnchorElement>) => (
        <a
          {...props}
          href={href}
          data-testid="report-result-markdown-link"
          className="text-primary hover:bg-primary/10 rounded px-0.5 underline underline-offset-4 font-medium transition-colors"
          onClick={(event) => {
            void handleResultLinkClick(event, href ?? '');
          }}
        >
          {children}
        </a>
      ),
    }),
    [handleResultLinkClick],
  );

  return (
    <div
      data-testid="tool-structured-report-result"
      className={cn(
        'rounded-lg border border-border bg-card/90 p-4 space-y-4 text-card-foreground shadow-xs',
        statusConfig.containerBorder,
      )}
    >
      {/* Header */}
      <div className="flex items-center justify-between gap-3 border-b border-border/70 pb-3">
        <div className="flex items-center gap-2.5 min-w-0">
          <StatusIcon className="h-5 w-5 shrink-0 text-foreground" />
          <h3 className="font-semibold text-sm sm:text-base text-foreground truncate">
            {displayTitle}
          </h3>
        </div>
        <span
          className={cn(
            'inline-flex items-center gap-1 rounded-full border px-2.5 py-0.5 text-xs font-semibold shrink-0',
            statusConfig.badgeClass,
          )}
        >
          {statusConfig.label}
        </span>
      </div>

      {/* Acceptance Criteria & Verification Proof (optional) */}
      {data.criteria || data.proof ? (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3 text-xs">
          {data.criteria ? (
            <div className="rounded-md border border-border/70 bg-muted/40 p-2.5 space-y-1.5">
              <div className="flex items-center gap-1.5 font-medium text-muted-foreground uppercase tracking-wider text-[11px]">
                <ClipboardCheck className="h-3.5 w-3.5 text-foreground" />
                <span>
                  {t(
                    'agent.toolStructured.acceptanceCriteria',
                    'Acceptance criteria',
                  )}
                </span>
              </div>
              <div className="text-foreground/90 whitespace-pre-wrap leading-relaxed font-sans text-xs">
                {data.criteria}
              </div>
            </div>
          ) : null}

          {data.proof ? (
            <div className="rounded-md border border-border/70 bg-muted/40 p-2.5 space-y-1.5">
              <div className="flex items-center gap-1.5 font-medium text-muted-foreground uppercase tracking-wider text-[11px]">
                <ShieldCheck className="h-3.5 w-3.5 text-foreground" />
                <span>
                  {t(
                    'agent.toolStructured.verificationProof',
                    'Verification proof',
                  )}
                </span>
              </div>
              <div className="text-foreground/90 whitespace-pre-wrap leading-relaxed font-sans text-xs">
                {data.proof}
              </div>
            </div>
          ) : null}
        </div>
      ) : null}

      {/* Result Markdown Content */}
      <div className="space-y-1.5 pt-1">
        <div className="text-[11px] font-medium text-muted-foreground uppercase tracking-wider">
          {t('agent.toolStructured.resultSummary', 'Outcome')}
        </div>
        <div className="prose dark:prose-invert max-w-none text-sm leading-relaxed rounded-md bg-muted/20 border border-border/50 p-3.5">
          <ReactMarkdown
            remarkPlugins={[remarkGfm]}
            components={markdownComponents}
          >
            {data.result}
          </ReactMarkdown>
        </div>
      </div>

      {/* Deliverables Section */}
      {data.deliverables && data.deliverables.length > 0 ? (
        <div className="space-y-2 pt-2 border-t border-border/70">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-1.5 text-xs font-semibold text-foreground">
              <Package className="h-4 w-4 text-foreground" />
              <span>
                {t(
                  'agent.toolStructured.deliverablesCount',
                  'Deliverables ({{count}})',
                  {
                    count: data.deliverables.length,
                  },
                )}
              </span>
            </div>
          </div>

          <div className="space-y-2">
            {data.deliverables.map((item) => (
              <DeliverableFileActions
                key={item.path}
                item={item}
                sessionId={activeSessionId}
              />
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
};
