import { useCallback, useEffect, useRef, useState } from 'react';
import {
  Card,
  CardHeader,
  CardTitle,
  CardContent,
  CardFooter,
} from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import {
  ShieldAlert,
  Check,
  X,
  Code,
  Loader2,
  Shield,
  TriangleAlert,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { PendingApproval } from '@/context/AgentSessionContext';
import { ScrollArea } from '@/components/ui/scroll-area';

import type { ExecutionMode } from '@/context/agent-session/types';

interface PendingApprovalWidgetProps {
  approvals: PendingApproval[];
  executionMode: ExecutionMode;
  onRespond: (toolCallId: string, approved: boolean) => Promise<void>;
}

function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  const tag = target.tagName;
  return (
    tag === 'INPUT' ||
    tag === 'TEXTAREA' ||
    tag === 'SELECT' ||
    target.isContentEditable
  );
}

/** Prefer hard approvals; otherwise use the first pending item. */
function pickPriorityApproval(
  approvals: PendingApproval[],
): PendingApproval | undefined {
  return (
    approvals.find((approval) => approval.approvalKind === 'hard') ??
    approvals[0]
  );
}

export function PendingApprovalWidget({
  approvals,
  executionMode,
  onRespond,
}: PendingApprovalWidgetProps) {
  const { t } = useTranslation();
  const [submittingId, setSubmittingId] = useState<string | null>(null);
  const firstCardRef = useRef<HTMLDivElement | null>(null);
  const submittingIdRef = useRef<string | null>(null);

  submittingIdRef.current = submittingId;

  const handleResponse = useCallback(
    async (toolCallId: string, approved: boolean) => {
      if (submittingIdRef.current !== null) {
        return;
      }
      setSubmittingId(toolCallId);
      try {
        await onRespond(toolCallId, approved);
      } finally {
        setSubmittingId(null);
      }
    },
    [onRespond],
  );

  const firstToolCallId = approvals[0]?.toolCallId;

  useEffect(() => {
    if (!firstToolCallId || !firstCardRef.current) {
      return;
    }

    const active = document.activeElement;
    // Don't steal focus from chat input or other interactive controls.
    const canAutofocus =
      !active ||
      active === document.body ||
      firstCardRef.current.contains(active);

    if (canAutofocus) {
      firstCardRef.current.focus();
    }
  }, [firstToolCallId]);

  useEffect(() => {
    if (!approvals || approvals.length === 0) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (submittingIdRef.current !== null) {
        return;
      }
      if (isTypingTarget(event.target)) {
        return;
      }

      const target = pickPriorityApproval(approvals);
      if (!target) {
        return;
      }

      if (event.key === 'Enter') {
        event.preventDefault();
        void handleResponse(target.toolCallId, true);
        return;
      }

      if (event.key === 'Escape') {
        event.preventDefault();
        void handleResponse(target.toolCallId, false);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [approvals, handleResponse]);

  if (!approvals || approvals.length === 0) return null;

  return (
    <div className="flex flex-col gap-4 w-full max-w-full">
      {approvals.map((approval, index) => {
        let argsParsed;
        try {
          argsParsed = JSON.parse(approval.arguments);
        } catch {
          argsParsed = approval.arguments;
        }

        const isSubmitting = submittingId === approval.toolCallId;
        const isHardApproval = approval.approvalKind === 'hard';
        const title = isHardApproval
          ? t('agent.approval.hardTitle', 'Hard Approval Required')
          : t('agent.approval.title', 'Approval Required');
        const subtitle = isHardApproval
          ? executionMode === 'yolo'
            ? t(
                'agent.approval.hardDescriptionYolo',
                'YOLO mode is on, but this high-risk action still requires manual approval.',
              )
            : t(
                'agent.approval.hardDescription',
                'This high-risk action requires explicit approval.',
              )
          : t(
              'agent.approval.description',
              'The agent wants to execute this tool.',
            );
        const containerClass = isHardApproval
          ? 'border-destructive/50 bg-destructive/5 shadow-md'
          : 'border-warning/50 bg-warning/5 shadow-md';
        const iconContainerClass = isHardApproval
          ? 'w-8 h-8 rounded-full bg-destructive/15 flex items-center justify-center text-destructive flex-shrink-0'
          : 'w-8 h-8 rounded-full bg-warning/20 flex items-center justify-center text-warning flex-shrink-0';
        const ApproveIcon = isHardApproval ? Shield : Check;
        const approvalKindLabel = isHardApproval
          ? t('agent.approval.hardBadge', 'Hard approval')
          : t('agent.approval.standardBadge', 'Standard approval');

        const card = (
          <Card className={containerClass}>
            <CardHeader className="pb-3 flex flex-row items-center gap-3">
              <div className={iconContainerClass}>
                {isHardApproval ? (
                  <TriangleAlert size={18} />
                ) : (
                  <ShieldAlert size={18} />
                )}
              </div>
              <div className="flex-1 overflow-hidden">
                <CardTitle className="text-sm font-semibold flex items-center gap-2">
                  {title}
                  <Badge
                    variant="outline"
                    className={
                      isHardApproval
                        ? 'text-xs border-destructive/30 bg-destructive/10 text-destructive'
                        : 'text-xs bg-background/50'
                    }
                  >
                    {approvalKindLabel}
                  </Badge>
                  <Badge
                    variant="outline"
                    className="font-mono text-xs bg-background/50"
                  >
                    {approval.toolName}
                  </Badge>
                </CardTitle>
                <div className="text-xs text-muted-foreground mt-1 line-clamp-1">
                  {subtitle}
                </div>
              </div>
            </CardHeader>
            <CardContent className="pb-4">
              <div className="rounded-md bg-background/50 border border-border/50 overflow-hidden">
                <div className="bg-muted/30 px-3 py-1.5 flex items-center gap-2 border-b border-border/50">
                  <Code size={12} className="text-muted-foreground" />
                  <span className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">
                    {t('agent.approval.arguments', 'Arguments')}
                  </span>
                </div>
                <ScrollArea className="max-h-32">
                  <div className="p-3 text-xs font-mono whitespace-pre-wrap break-all">
                    {typeof argsParsed === 'object'
                      ? JSON.stringify(argsParsed, null, 2)
                      : String(argsParsed)}
                  </div>
                </ScrollArea>
              </div>
            </CardContent>
            <CardFooter className="flex justify-end gap-2 pt-0">
              <Button
                variant="outline"
                size="sm"
                onClick={() => handleResponse(approval.toolCallId, false)}
                disabled={isSubmitting}
                className="hover:bg-destructive/10 hover:text-destructive hover:border-destructive/30"
              >
                {isSubmitting && submittingId === approval.toolCallId ? (
                  <Loader2 className="w-3.5 h-3.5 mr-1.5 animate-spin" />
                ) : (
                  <X className="w-3.5 h-3.5 mr-1.5" />
                )}
                {t('agent.approval.reject', 'Reject')}
              </Button>
              <Button
                size="sm"
                onClick={() => handleResponse(approval.toolCallId, true)}
                disabled={isSubmitting}
                className={
                  isHardApproval
                    ? 'bg-destructive hover:bg-destructive/90 text-destructive-foreground'
                    : 'bg-warning hover:bg-warning/90 text-warning-foreground'
                }
              >
                {isSubmitting && submittingId === approval.toolCallId ? (
                  <Loader2 className="w-3.5 h-3.5 mr-1.5 animate-spin" />
                ) : (
                  <ApproveIcon className="w-3.5 h-3.5 mr-1.5" />
                )}
                {isHardApproval
                  ? t('agent.approval.hardApprove', 'Approve high-risk action')
                  : t('agent.approval.approve', 'Approve')}
              </Button>
            </CardFooter>
          </Card>
        );

        if (index === 0) {
          return (
            <div
              key={approval.toolCallId}
              ref={firstCardRef}
              tabIndex={0}
              className="outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 rounded-xl"
            >
              {card}
            </div>
          );
        }

        return <div key={approval.toolCallId}>{card}</div>;
      })}
    </div>
  );
}
