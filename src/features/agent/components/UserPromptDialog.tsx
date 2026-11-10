import { useCallback, useEffect, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
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
import {
  AskUserQuestions,
  buildAskUserAnswers,
  type QuestionDraft,
} from './AskUserQuestions';

const logger = getLogger('UserPromptDialog');

export interface UserPromptDialogProps {
  sessionId: string;
  promptState: PendingUserPromptSummary | null;
}

/**
 * One dialog for both prompt kinds (#2543). The payload is tagged with `kind`,
 * so the form switches on it: a `secret` masks the input, a `question` shows
 * one section per `ui__askUser` question. The typed value is sent to the
 * backend only — never stored in the transcript, an event, or a log.
 */
export default function UserPromptDialog({
  sessionId,
  promptState,
}: UserPromptDialogProps) {
  const { t } = useTranslation();
  const [value, setValue] = useState('');
  const [drafts, setDrafts] = useState<Record<string, QuestionDraft>>({});
  const [isSubmitting, setIsSubmitting] = useState(false);

  const requestId = promptState?.requestId ?? null;
  const payload = promptState?.payload ?? null;
  let submitValue: string | null = value;
  if (payload?.kind === 'question') {
    const answers = buildAskUserAnswers(payload.questions, drafts);
    submitValue = answers ? JSON.stringify(answers) : null;
  }

  useEffect(() => {
    setValue('');
    setDrafts({});
    setIsSubmitting(false);
  }, [requestId]);

  const handleDraftChange = useCallback((id: string, draft: QuestionDraft) => {
    setDrafts((current) => ({ ...current, [id]: draft }));
  }, []);

  const handleCancel = useCallback(async () => {
    if (!requestId || isSubmitting) {
      return;
    }

    setIsSubmitting(true);
    try {
      await cancelUserPrompt(sessionId, requestId);
    } catch (error) {
      logger.error('Failed to cancel user prompt', error);
      toast.error(t('agent.userPromptDialog.cancelFailed'));
    } finally {
      setIsSubmitting(false);
    }
  }, [isSubmitting, requestId, sessionId, t]);

  const handleSubmit = useCallback(
    async (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();

      if (!requestId || isSubmitting || submitValue === null) {
        return;
      }

      setIsSubmitting(true);
      try {
        await submitUserPrompt(sessionId, requestId, submitValue);
        setValue('');
      } catch (error) {
        logger.error('Failed to submit user prompt', error);
        toast.error(t('agent.userPromptDialog.submitFailed'));
      } finally {
        setIsSubmitting(false);
      }
    },
    [isSubmitting, requestId, sessionId, submitValue, t],
  );

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
              ? t('agent.userPromptDialog.titleSecret')
              : t('agent.userPromptDialog.titleQuestion')}
          </DialogTitle>
          <DialogDescription>
            {payload?.kind === 'secret'
              ? payload.label
              : t('agent.userPromptDialog.waitingAnswer')}
          </DialogDescription>
        </DialogHeader>

        {promptState && payload ? (
          <form className="space-y-4" onSubmit={handleSubmit}>
            {payload.kind === 'question' ? (
              <AskUserQuestions
                questions={payload.questions}
                drafts={drafts}
                disabled={isSubmitting}
                onChange={handleDraftChange}
              />
            ) : (
              <div className="space-y-2">
                <Label htmlFor="user-prompt-input">
                  {payload.variableName}
                </Label>
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
                {payload.reuseNote ? (
                  <p className="text-muted-foreground text-xs">
                    {payload.reuseNote}
                  </p>
                ) : null}
              </div>
            )}

            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => void handleCancel()}
                disabled={isSubmitting}
              >
                {t('common.cancel')}
              </Button>
              <Button
                type="submit"
                disabled={isSubmitting || submitValue === null}
              >
                {t('agent.userPromptDialog.submit')}
              </Button>
            </DialogFooter>
          </form>
        ) : null}
      </DialogContent>
    </Dialog>
  );
}
