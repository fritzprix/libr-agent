import { useState, useCallback, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { AlertTriangle, ChevronDown, Loader2, Play } from 'lucide-react';
import { toast } from 'sonner';

import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
  Button,
} from '@/components/ui';
import { getBackendErrorMessage } from '@/lib/backend/errors';
import { waitForDockerReady } from '@/lib/backend/dockerHealth';
import {
  isDockerDesktopLaunchSupported,
  startDockerDesktop,
} from '@/lib/backend/workspace';

interface DockerErrorModalProps {
  isOpen: boolean;
  onClose: () => void;
  onRetry: () => void;
  errorDetails?: string | null;
}

export function DockerErrorModal({
  isOpen,
  onClose,
  onRetry,
  errorDetails,
}: DockerErrorModalProps) {
  const { t } = useTranslation();
  const [isStarting, setIsStarting] = useState(false);
  const [isWaitingForEngine, setIsWaitingForEngine] = useState(false);
  const [launchSupported, setLaunchSupported] = useState<boolean | null>(null);
  const [showDetails, setShowDetails] = useState(false);
  const waitAbortRef = useRef<AbortController | null>(null);

  useEffect(() => {
    if (!isOpen) {
      waitAbortRef.current?.abort();
      waitAbortRef.current = null;
      setLaunchSupported(null);
      setIsWaitingForEngine(false);
      setShowDetails(false);
      return;
    }

    let cancelled = false;
    void isDockerDesktopLaunchSupported()
      .then((supported) => {
        if (!cancelled) {
          setLaunchSupported(supported);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setLaunchSupported(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [isOpen]);

  const waitForEngineThenRetry = useCallback(async () => {
    waitAbortRef.current?.abort();
    const controller = new AbortController();
    waitAbortRef.current = controller;

    setIsWaitingForEngine(true);
    try {
      const ready = await waitForDockerReady({ signal: controller.signal });
      if (controller.signal.aborted) {
        return;
      }
      if (ready) {
        onClose();
        onRetry();
      } else {
        toast.error(t('agent.draft.dockerStillNotReady'));
      }
    } finally {
      if (waitAbortRef.current === controller) {
        waitAbortRef.current = null;
      }
      setIsWaitingForEngine(false);
    }
  }, [onClose, onRetry, t]);

  const handleStartDocker = useCallback(async () => {
    setIsStarting(true);
    try {
      await startDockerDesktop();
      toast.success(t('agent.draft.dockerStartSuccess'));
      await waitForEngineThenRetry();
    } catch (err) {
      const errorMsg = getBackendErrorMessage(err);
      toast.error(t('agent.draft.dockerStartError', { error: errorMsg }));
    } finally {
      setIsStarting(false);
    }
  }, [t, waitForEngineThenRetry]);

  const handleRetry = useCallback(() => {
    void waitForEngineThenRetry();
  }, [waitForEngineThenRetry]);

  const isBusy = isStarting || isWaitingForEngine;

  return (
    <Dialog
      open={isOpen}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent className="w-[calc(100%-2rem)] max-w-md sm:max-w-md max-h-[85vh] overflow-x-hidden overflow-y-auto border-border bg-background shadow-2xl">
        <DialogHeader className="items-center gap-3 text-center sm:text-center">
          <div className="flex h-12 w-12 items-center justify-center rounded-full bg-amber-500/10 text-amber-500 animate-pulse">
            <AlertTriangle className="h-6 w-6" />
          </div>
          <DialogTitle className="text-xl font-bold tracking-tight text-foreground">
            {t('agent.draft.dockerErrorTitle')}
          </DialogTitle>
          <DialogDescription className="text-sm text-muted-foreground">
            {t('agent.draft.dockerErrorDescription')}
          </DialogDescription>
        </DialogHeader>

        <div className="min-w-0 space-y-3 rounded-xl border border-border/40 bg-muted/[0.08] p-4 text-left text-sm text-foreground/90">
          <p className="font-semibold text-muted-foreground text-xs uppercase tracking-wider mb-2">
            {t('agent.draft.dockerTroubleshootingSteps')}
          </p>
          <div className="space-y-2.5">
            <div className="flex items-start gap-2.5">
              <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-primary/10 text-xs font-semibold text-primary">
                1
              </span>
              <span className="min-w-0 break-words leading-normal">
                {t('agent.draft.dockerErrorStep1')}
              </span>
            </div>
            <div className="flex items-start gap-2.5">
              <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-primary/10 text-xs font-semibold text-primary">
                2
              </span>
              <span className="min-w-0 break-words leading-normal">
                {t('agent.draft.dockerErrorStep2')}
              </span>
            </div>
            <div className="flex items-start gap-2.5">
              <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-primary/10 text-xs font-semibold text-primary">
                3
              </span>
              <span className="min-w-0 break-words leading-normal">
                {t('agent.draft.dockerErrorStep3')}
              </span>
            </div>
          </div>

          {errorDetails ? (
            <div className="mt-4 pt-3 border-t border-border/40">
              <button
                type="button"
                onClick={() => setShowDetails((prev) => !prev)}
                aria-expanded={showDetails}
                className="flex w-full items-center justify-between gap-2 rounded text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground transition-colors hover:text-foreground"
              >
                <span className="min-w-0 truncate">
                  {showDetails
                    ? t('agent.draft.dockerHideErrorDetails')
                    : t('agent.draft.dockerShowErrorDetails')}
                </span>
                <ChevronDown
                  className={`h-4 w-4 shrink-0 transition-transform ${
                    showDetails ? 'rotate-180' : ''
                  }`}
                />
              </button>
              {showDetails ? (
                <pre className="mt-2 max-h-40 w-full min-w-0 overflow-auto whitespace-pre-wrap break-all rounded bg-muted/60 p-2 font-mono text-[10px] text-muted-foreground leading-relaxed">
                  {errorDetails}
                </pre>
              ) : null}
            </div>
          ) : null}
        </div>

        <DialogFooter className="flex flex-col sm:flex-row sm:flex-wrap gap-2">
          <Button
            type="button"
            variant="outline"
            onClick={onClose}
            disabled={isBusy}
            className="w-full sm:w-auto"
          >
            {t('common:cancel', 'Cancel')}
          </Button>

          {launchSupported ? (
            <Button
              type="button"
              variant="secondary"
              onClick={() => void handleStartDocker()}
              disabled={isBusy}
              className="w-full sm:w-auto gap-2"
            >
              {isStarting ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Play className="h-4 w-4 fill-current" />
              )}
              {isStarting
                ? t('agent.draft.dockerStarting')
                : t('agent.draft.dockerStartButton')}
            </Button>
          ) : null}

          <Button
            type="button"
            variant="default"
            onClick={handleRetry}
            disabled={isBusy}
            autoFocus
            className="w-full sm:w-auto gap-2"
          >
            {isWaitingForEngine ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : null}
            {isWaitingForEngine
              ? t('agent.draft.dockerWaitingForEngine')
              : t('agent.draft.dockerRetryButton')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
