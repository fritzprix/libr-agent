import { useCallback, useEffect, useState, type FormEvent } from 'react';
import {
  cancelInteractiveShellInput,
  submitInteractiveShellInput,
} from '@/lib/backend/agent-commands';
import type { PendingInteractiveShellPrompt } from '@/context/agent-session/types';
import { toast } from 'sonner';
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

const logger = getLogger('InteractiveShellPromptDialog');

export interface InteractiveShellPromptDialogProps {
  sessionId: string;
  promptState: PendingInteractiveShellPrompt | null;
}

export default function InteractiveShellPromptDialog({
  sessionId,
  promptState,
}: InteractiveShellPromptDialogProps) {
  const [inputValue, setInputValue] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    setInputValue('');
    setIsSubmitting(false);
  }, [promptState?.executionId]);

  const handleCancel = useCallback(async () => {
    if (!promptState || isSubmitting) {
      return;
    }

    const { executionId } = promptState;
    setIsSubmitting(true);
    try {
      await cancelInteractiveShellInput(sessionId, executionId);
    } catch (error) {
      logger.error('Failed to cancel interactive shell prompt', error);
      toast.error('Failed to cancel interactive prompt.');
    } finally {
      setIsSubmitting(false);
    }
  }, [isSubmitting, promptState, sessionId]);

  const handleSubmit = useCallback(
    async (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();

      if (!promptState || isSubmitting) {
        return;
      }

      const { executionId } = promptState;
      setIsSubmitting(true);
      try {
        await submitInteractiveShellInput(sessionId, executionId, inputValue);
        setInputValue('');
      } catch (error) {
        logger.error('Failed to submit interactive shell input', error);
        toast.error('Failed to submit interactive input.');
      } finally {
        setIsSubmitting(false);
      }
    },
    [inputValue, isSubmitting, promptState, sessionId],
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
          <DialogTitle>Interactive shell input required</DialogTitle>
          <DialogDescription>
            {promptState?.command ?? 'A shell command'} is waiting for local
            user input.
          </DialogDescription>
        </DialogHeader>

        {promptState ? (
          <form className="space-y-4" onSubmit={handleSubmit}>
            <div className="space-y-2">
              <Label htmlFor="interactive-shell-input">
                {promptState.prompt}
              </Label>
              <Input
                autoFocus
                id="interactive-shell-input"
                type={promptState.inputType}
                value={inputValue}
                onChange={(event) => setInputValue(event.target.value)}
                autoComplete="off"
                spellCheck={false}
                disabled={isSubmitting}
              />
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
