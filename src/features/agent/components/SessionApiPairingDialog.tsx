import React, { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { toast } from 'sonner';
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
import { Badge } from '@/components/ui';
import { KeyRound, Loader2, ShieldCheck, AlertCircle } from 'lucide-react';
import {
  approveSessionApiPairing,
  getPendingSessionApiPairing,
  rejectSessionApiPairing,
  type PendingPairingInfo,
} from '@/lib/backend/session-api-keys';
import { getLogger } from '@/lib/logger';

const logger = getLogger('SessionApiPairingDialog');

export function SessionApiPairingDialog() {
  const { t } = useTranslation('common');
  const [pending, setPending] = useState<PendingPairingInfo | null>(null);
  const [code, setCode] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [timeLeft, setTimeLeft] = useState<number>(0);

  // Check on mount
  useEffect(() => {
    let mounted = true;
    void getPendingSessionApiPairing().then((info) => {
      if (mounted && info) {
        setPending(info);
        setTimeLeft(info.expiresInSeconds);
      }
    });

    const unlistenPromise = listen<PendingPairingInfo>(
      'session_api_pairing_requested',
      (event) => {
        if (!mounted) return;
        setPending(event.payload);
        setTimeLeft(event.payload.expiresInSeconds);
        setCode('');
        setErrorMessage(null);
      },
    );

    return () => {
      mounted = false;
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  // Countdown timer
  useEffect(() => {
    if (!pending || timeLeft <= 0) return;
    const interval = setInterval(() => {
      setTimeLeft((prev) => {
        if (prev <= 1) {
          setPending(null);
          return 0;
        }
        return prev - 1;
      });
    }, 1000);
    return () => clearInterval(interval);
  }, [pending, timeLeft]);

  const handleApprove = useCallback(
    async (e?: React.FormEvent) => {
      e?.preventDefault();
      if (!code.trim() || isSubmitting) return;

      setIsSubmitting(true);
      setErrorMessage(null);
      try {
        const result = await approveSessionApiPairing(code.trim());
        toast.success(
          t(
            'settings.sessionApi.pairingSuccess',
            'Successfully paired with {{name}}',
            { name: result.name },
          ),
        );
        setPending(null);
        setCode('');
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        logger.error('Failed to approve pairing', { err });
        setErrorMessage(message);
        if (message.includes('cancelled') || message.includes('expired')) {
          setPending(null);
        }
      } finally {
        setIsSubmitting(false);
      }
    },
    [code, isSubmitting, t],
  );

  const handleReject = useCallback(async () => {
    if (isSubmitting) return;
    setIsSubmitting(true);
    try {
      await rejectSessionApiPairing();
      toast.info(
        t('settings.sessionApi.pairingRejected', 'Pairing request rejected'),
      );
    } catch (err) {
      logger.error('Failed to reject pairing', { err });
    } finally {
      setIsSubmitting(false);
      setPending(null);
      setCode('');
      setErrorMessage(null);
    }
  }, [isSubmitting, t]);

  if (!pending) return null;

  return (
    <Dialog
      open={Boolean(pending)}
      onOpenChange={(open) => {
        if (!open) {
          void handleReject();
        }
      }}
    >
      <DialogContent className="max-w-md">
        <form onSubmit={handleApprove}>
          <DialogHeader className="gap-1">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <KeyRound className="h-5 w-5 text-primary" />
                <DialogTitle>
                  {t(
                    'settings.sessionApi.pairingTitle',
                    'Agent Pairing Request',
                  )}
                </DialogTitle>
              </div>
              <Badge variant="outline" className="font-mono text-xs">
                {Math.floor(timeLeft / 60)}:
                {(timeLeft % 60).toString().padStart(2, '0')}
              </Badge>
            </div>
            <DialogDescription className="pt-2 text-sm text-foreground">
              {t(
                'settings.sessionApi.pairingDescription',
                'External agent "{{name}}" is requesting Session API access. Enter the 6-character code shown on the agent screen to authorize.',
                { name: pending.name },
              )}
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-3 py-4">
            <div className="space-y-1">
              <label
                htmlFor="pairing-code-input"
                className="text-xs font-medium text-muted-foreground"
              >
                {t(
                  'settings.sessionApi.pairingCodeLabel',
                  'Pairing Code (xxx-yyy)',
                )}
              </label>
              <Input
                id="pairing-code-input"
                autoFocus
                placeholder="e.g. abc-123"
                value={code}
                onChange={(e) => {
                  setCode(e.target.value);
                  setErrorMessage(null);
                }}
                disabled={isSubmitting}
                className="font-mono text-center text-lg tracking-widest uppercase"
                maxLength={7}
              />
            </div>

            {errorMessage && (
              <div className="flex items-center gap-2 rounded-md bg-destructive/10 p-2 text-xs text-destructive">
                <AlertCircle className="h-4 w-4 shrink-0" />
                <span>{errorMessage}</span>
              </div>
            )}
          </div>

          <DialogFooter className="gap-2 sm:gap-0">
            <Button
              type="button"
              variant="outline"
              onClick={handleReject}
              disabled={isSubmitting}
            >
              {t('common.reject', 'Reject')}
            </Button>
            <Button
              type="submit"
              disabled={!code.trim() || isSubmitting}
              className="gap-2"
            >
              {isSubmitting ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <ShieldCheck className="h-4 w-4" />
              )}
              {t('common.approve', 'Approve')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
