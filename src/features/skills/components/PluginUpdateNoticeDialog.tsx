import { useTranslation } from 'react-i18next';
import { Loader2 } from 'lucide-react';
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

interface PluginUpdateNoticeDialogProps {
  /** The plugin being updated; non-null opens the dialog (#2927 D6). */
  pendingUpdateUrl: string | null;
  isBusy: boolean;
  onOpenChange: (open: boolean) => void;
  /** One Agree approves the whole update, hooks and MCP servers included. */
  onAgree: () => Promise<void> | void;
}

/**
 * The #2927 D6 update notice: a short, calm warning that an update can
 * change MCP servers and hooks (a security risk), closed by ONE Agree that
 * approves the whole update — changed hooks and MCP servers included.
 * Shared by the Skills view and the chat-input `/plugin update` flow.
 */
export function PluginUpdateNoticeDialog({
  pendingUpdateUrl,
  isBusy,
  onOpenChange,
  onAgree,
}: PluginUpdateNoticeDialogProps) {
  const { t } = useTranslation('common');

  return (
    <AlertDialog open={pendingUpdateUrl !== null} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>
            {t(
              'settings.skills.pluginUpdateNoticeTitle',
              'Update this plugin?',
            )}
          </AlertDialogTitle>
          <AlertDialogDescription>
            {t(
              'settings.skills.pluginUpdateNoticeBody',
              'An update re-downloads the plugin. It can change the plugin MCP servers and hooks, and those can run code on your machine. Agreeing approves this whole update, including any changed MCP servers and hooks.',
            )}
          </AlertDialogDescription>
        </AlertDialogHeader>
        {pendingUpdateUrl ? (
          <p className="break-all text-xs text-muted-foreground">
            {pendingUpdateUrl}
          </p>
        ) : null}
        <AlertDialogFooter>
          <AlertDialogCancel disabled={isBusy}>
            {t('common.cancel', 'Cancel')}
          </AlertDialogCancel>
          <AlertDialogAction
            disabled={isBusy}
            onClick={(event) => {
              // Keep the dialog open while the update runs; the caller
              // closes it when the flow finishes or needs the conflict
              // dialog instead.
              event.preventDefault();
              void onAgree();
            }}
          >
            {isBusy ? (
              <Loader2 className="w-4 h-4 mr-2 animate-spin" />
            ) : null}
            {t('settings.skills.pluginUpdateAgree', 'Agree & update')}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
