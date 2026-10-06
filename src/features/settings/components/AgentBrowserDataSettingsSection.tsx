import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { Button } from '@/components/ui';
import { clearAgentBrowserData } from '@/lib/backend/browser';

export function AgentBrowserDataSettingsSection() {
  const { t } = useTranslation('common');
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [clearing, setClearing] = useState(false);

  const handleClear = async () => {
    setClearing(true);
    try {
      await clearAgentBrowserData();
      toast.success(
        t(
          'settings.system.agentBrowser.clearSuccess',
          'Agent browser data cleared',
        ),
        {
          description: t(
            'settings.system.agentBrowser.clearSuccessBody',
            'Saved logins inside LibrAgent’s agent browser were removed. Everyday Chrome is unchanged.',
          ),
        },
      );
      setConfirmOpen(false);
    } catch (error) {
      const message =
        error instanceof Error
          ? error.message
          : t(
              'settings.system.agentBrowser.clearError',
              'Could not clear agent browser data. Close any open agent browser windows and try again.',
            );
      toast.error(
        t('settings.system.agentBrowser.clearErrorTitle', 'Clear failed'),
        { description: message },
      );
    } finally {
      setClearing(false);
    }
  };

  return (
    <div className="space-y-4 max-w-lg">
      <div>
        <h4 className="text-sm font-medium text-foreground">
          {t('settings.system.agentBrowser.title', 'Agent browser data')}
        </h4>
        <p className="mt-1 text-xs text-muted-foreground leading-relaxed">
          {t(
            'settings.system.agentBrowser.description',
            'LibrAgent saves logins in its built-in browser so new sessions stay signed in. This is separate from your everyday Chrome.',
          )}
        </p>
      </div>
      <Button
        type="button"
        variant="outline"
        className="h-8"
        disabled={clearing}
        onClick={() => setConfirmOpen(true)}
      >
        {clearing
          ? t('settings.system.agentBrowser.clearing', 'Clearing…')
          : t(
              'settings.system.agentBrowser.clearButton',
              'Clear agent browser data',
            )}
      </Button>

      <AlertDialog open={confirmOpen} onOpenChange={setConfirmOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t(
                'settings.system.agentBrowser.clearConfirmTitle',
                'Clear agent browser data?',
              )}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t(
                'settings.system.agentBrowser.clearConfirm',
                'This deletes cookies and logins stored for LibrAgent’s agent browser. Open browser sessions will be closed. Everyday Chrome is not affected.',
              )}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={clearing}>
              {t('common.cancel', 'Cancel')}
            </AlertDialogCancel>
            <AlertDialogAction
              disabled={clearing}
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
              onClick={(event) => {
                event.preventDefault();
                void handleClear();
              }}
            >
              {clearing
                ? t('settings.system.agentBrowser.clearing', 'Clearing…')
                : t(
                    'settings.system.agentBrowser.clearButton',
                    'Clear agent browser data',
                  )}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
