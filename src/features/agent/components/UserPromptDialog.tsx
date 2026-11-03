import { useCallback, useEffect, useState, type FormEvent } from 'react';
import { toast } from 'sonner';
import {
  cancelUserPrompt,
  submitUserPrompt,
} from '@/lib/backend/agent-commands';
import type { PendingUserPromptSummary } from '@/context/agent-session/types';
import { getLogger } from '@/lib/logger';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';

const logger = getLogger('UserPromptDialog');

export interface UserPromptDialogProps {
  sessionId: string;
  promptState: PendingUserPromptSummary | null;
}

/**
 * One dialog for both prompt kinds (#2543). The payload is tagged with `kind`,
 * so the form switches on it: a `secret` masks the input, a `question` shows a
 * free-text area. The typed value is sent to the backend only — never stored in
 * the transcript, an event, or a log.
 */
export default function UserPromptDialog({
  sessionId,
  promptState,
}: UserPromptDialogProps) {
  const [value, setValue] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);

  const requestId = promptState?.requestId ?? null;

  useEffect(() => {
    setValue('');
    setIsSubmitting(false);
  }, [requestId]);

  const handleCancel = useCallback(async () => {
    if (!requestId || isSubmitting) {
      return;
    }

    setIsSubmitting(true);
    try {
      await cancelUserPrompt(sessionId, requestId);
    } catch (error) {
      logger.error('Failed to cancel user prompt', error);
      toast.error('Failed to cancel the prompt.');
    } finally {
      setIsSubmitting(false);
    }
  }, [isSubmitting, requestId, sessionId]);

  const handleSubmit = useCallback(
    async (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();

      if (!requestId || isSubmitting) {
        return;
      }

      setIsSubmitting(true);
      try {
        await submitUserPrompt(sessionId, requestId, value);
        setValue('');
      } catch (error) {
        logger.error('Failed to submit user prompt', error);
        toast.error('Failed to send your answer.');
      } finally {
        setIsSubmitting(false);
      }
    },
    [isSubmitting, requestId, sessionId, value],
  );

  const payload = promptState?.payload ?? null;
  const isQuestion = payload?.kind === 'question';

  return (
    <Dialog
      open={promptState !== null}
      onOpenChange={(open) => {
        if (!open) {
          void handleCancel();
        }
      }}
    >
      <DialogContent
        showCloseButton={false}
        onEscapeKeyDown={(event) => {
          event.preventDefault();
          void handleCancel();
        }}
        onInteractOutside={(event) => {
          event.preventDefault();
        }}
      >
        <DialogHeader>
          <DialogTitle>
            {payload?.kind === 'secret'
              ? 'A password is needed'
              : (payload?.title ?? 'Your answer is needed')}
          </DialogTitle>
          {payload?.kind === 'secret' ? (
            <DialogDescription>{payload.label}</DialogDescription>
          ) : payload?.body ? (
            <DialogDescription>{payload.body}</DialogDescription>
          ) : (
            <DialogDescription>
              The agent is waiting for your answer.
            </DialogDescription>
          )}
        </DialogHeader>

        {promptState && payload ? (
          <form className="space-y-4" onSubmit={handleSubmit}>
            <div className="space-y-2">
              <Label htmlFor="user-prompt-input">
                {payload.kind === 'secret'
                  ? payload.variableName
                  : 'Your answer'}
              </Label>
              {isQuestion ? (
                <Textarea
                  autoFocus
                  id="user-prompt-input"
                  value={value}
                  onChange={(event) => setValue(event.target.value)}
                  disabled={isSubmitting}
                  rows={4}
                />
              ) : (
                <Input
                  autoFocus
                  id="user-prompt-input"
                  type="password"
                  value={value}
                  onChange={(event) => setValue(event.target.value)}
                  autoComplete="off"
                  spellCheck={false}
                  disabled={isSubmitting}
                />
              )}
              {payload.kind === 'secret' && payload.reuseNote ? (
                <p className="text-muted-foreground text-xs">
                  {payload.reuseNote}
                </p>
              ) : null}
            </div>

            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => void handleCancel()}
                disabled={isSubmitting}
              >
                Cancel
              </Button>
              <Button type="submit" disabled={isSubmitting}>
                Submit
              </Button>
            </DialogFooter>
          </form>
        ) : null}
      </DialogContent>
    </Dialog>
  );
}
