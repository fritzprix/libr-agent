import { useMemo, useState } from 'react';
import { X, Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/utils';
import { extractTextContent } from '@/lib/message-utils';
import type { Message } from '@/models/chat';
import { Button } from '@/components/ui';

interface LayeredPendingQueueProps {
  items?: Message[];
  onCancel: (messageId: string) => void | Promise<void>;
  disabled?: boolean;
}

function previewText(message: Message): string {
  const text = extractTextContent(message).trim();
  if (text.length > 0) {
    return text;
  }
  const attachmentCount = message.attachments?.length ?? 0;
  if (attachmentCount > 0) {
    return `${attachmentCount} attachment${attachmentCount === 1 ? '' : 's'}`;
  }
  return 'Queued prompt';
}

/**
 * FIFO waiting prompts stacked above the chat input.
 * Depth 0 = next to run (front). Higher depth = later in the queue.
 */
export function LayeredPendingQueue({
  items = [],
  onCancel,
  disabled = false,
}: LayeredPendingQueueProps) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const [cancellingId, setCancellingId] = useState<string | null>(null);

  const visibleItems = useMemo(() => items.slice(0, 5), [items]);

  if (visibleItems.length === 0) {
    return null;
  }

  const handleCancel = async (messageId: string) => {
    if (disabled || cancellingId) {
      return;
    }
    setCancellingId(messageId);
    try {
      await onCancel(messageId);
    } finally {
      setCancellingId(null);
    }
  };

  return (
    <div
      className="relative mb-2 px-1"
      onMouseEnter={() => setExpanded(true)}
      onMouseLeave={() => setExpanded(false)}
      onFocusCapture={() => setExpanded(true)}
      onBlurCapture={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
          setExpanded(false);
        }
      }}
    >
      <div
        className={cn(
          'relative transition-[min-height] duration-200 ease-out motion-reduce:transition-none',
          expanded ? 'min-h-[auto]' : 'min-h-[2.75rem]',
        )}
        style={
          expanded
            ? undefined
            : {
                minHeight: `${Math.min(44 + (visibleItems.length - 1) * 8, 72)}px`,
              }
        }
        role="status"
        aria-live="polite"
        aria-label={t('agent.pendingQueue.label', 'Queued prompts')}
      >
        {visibleItems.map((item, index) => {
          const depth = index;
          const isFront = depth === 0;
          const isCancelling = cancellingId === item.id;
          const preview = previewText(item);

          return (
            <div
              key={item.id}
              className={cn(
                'absolute inset-x-0 rounded-xl border border-border/50 bg-background/80 px-3 py-2 shadow-sm backdrop-blur-md supports-[backdrop-filter]:bg-background/55 transition-all duration-200 ease-out motion-reduce:transition-none',
                // Collapsed: only the front card receives pointer events so tap
                // expands / cancels on touch devices without hover.
                !expanded && !isFront && 'pointer-events-none',
                expanded && 'pointer-events-auto relative mb-1.5 last:mb-0',
              )}
              style={
                expanded
                  ? {
                      opacity: 1,
                      transform: 'none',
                      zIndex: visibleItems.length - depth,
                    }
                  : {
                      top: `${depth * 6}px`,
                      opacity: Math.max(0.45, 1 - depth * 0.18),
                      transform: `scale(${1 - depth * 0.03})`,
                      zIndex: visibleItems.length - depth,
                    }
              }
              onClick={() => {
                if (!expanded) {
                  setExpanded(true);
                }
              }}
            >
              <div className="flex items-start gap-2">
                <div className="min-w-0 flex-1">
                  <div className="mb-0.5 flex items-center gap-2 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
                    <span>
                      {isFront
                        ? t('agent.pendingQueue.next', 'Next')
                        : t('agent.pendingQueue.queued', 'Queued')}
                    </span>
                    <span className="text-muted-foreground/70">
                      #{index + 1}
                    </span>
                  </div>
                  <p className="truncate text-sm text-foreground/90" title={preview}>
                    {preview}
                  </p>
                </div>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className={cn(
                    'h-7 w-7 shrink-0 text-muted-foreground hover:text-destructive',
                    !expanded && !isFront && 'opacity-0',
                  )}
                  disabled={disabled || !!cancellingId}
                  aria-label={t(
                    'agent.pendingQueue.cancelNamed',
                    'Cancel queued prompt: {{preview}}',
                    { preview },
                  )}
                  onClick={(event) => {
                    event.stopPropagation();
                    void handleCancel(item.id);
                  }}
                >
                  {isCancelling ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <X className="h-3.5 w-3.5" />
                  )}
                </Button>
              </div>
            </div>
          );
        })}
      </div>
      {items.length > visibleItems.length ? (
        <p className="mt-1 text-center text-[10px] text-muted-foreground">
          {t('agent.pendingQueue.more', '+{{count}} more', {
            count: items.length - visibleItems.length,
          })}
        </p>
      ) : null}
    </div>
  );
}
