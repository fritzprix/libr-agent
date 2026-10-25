import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { Button } from '@/components/ui';
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible';
import {
  getExtensionBridgeStatus,
  getExtensionUnpackedPath,
  type ExtensionBridgeStatus,
} from '@/lib/backend/browser';
import { openExternalUrl } from '@/lib/backend/utils';

type UserFacingState = 'checking' | 'connected' | 'waiting_extension';

const CHROME_WEB_STORE_URL =
  'https://chromewebstore.google.com/detail/libragent-browser-bridge/eponcjengahlpkgbaolhemhmckopmfbe';

export function ExtensionBridgeSettingsSection() {
  const { t } = useTranslation('common');
  const [status, setStatus] = useState<ExtensionBridgeStatus | null>(null);
  const [unpackedPath, setUnpackedPath] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [pathError, setPathError] = useState<string | null>(null);
  const [advancedOpen, setAdvancedOpen] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const next = await getExtensionBridgeStatus();
      setStatus(next);
    } catch (error) {
      const message =
        error instanceof Error
          ? error.message
          : t(
              'settings.system.extensionBridge.statusError',
              'Could not read the Chrome connection status.',
            );
      toast.error(
        t(
          'settings.system.extensionBridge.statusErrorTitle',
          'Chrome connection unavailable',
        ),
        { description: message },
      );
    } finally {
      setLoading(false);
    }

    try {
      const path = await getExtensionUnpackedPath();
      setUnpackedPath(path);
      setPathError(null);
    } catch (error) {
      setUnpackedPath(null);
      setPathError(
        error instanceof Error
          ? error.message
          : t(
              'settings.system.extensionBridge.pathError',
              'Could not find the local install folder.',
            ),
      );
    }
  }, [t]);

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => {
      void refresh();
    }, 5000);
    return () => window.clearInterval(timer);
  }, [refresh]);

  const copyPath = async () => {
    if (!unpackedPath) {
      return;
    }
    try {
      await navigator.clipboard.writeText(unpackedPath);
      toast.success(
        t(
          'settings.system.extensionBridge.pathCopied',
          'Install folder path copied',
        ),
      );
    } catch {
      toast.error(
        t(
          'settings.system.extensionBridge.pathCopyFailed',
          'Could not copy the path',
        ),
      );
    }
  };

  const openStore = async () => {
    try {
      await openExternalUrl(CHROME_WEB_STORE_URL);
    } catch {
      window.open(CHROME_WEB_STORE_URL, '_blank', 'noopener,noreferrer');
    }
  };

  const connected = status?.connected === true;
  const userState: UserFacingState = loading
    ? 'checking'
    : connected
      ? 'connected'
      : 'waiting_extension';

  const statusLabel =
    userState === 'checking'
      ? t('settings.system.extensionBridge.checking', 'Checking…')
      : userState === 'connected'
        ? t(
            'settings.system.extensionBridge.connectedHeadline',
            'Connected to your Chrome',
          )
        : t(
            'settings.system.extensionBridge.waitingHeadline',
            'Waiting for the Chrome extension',
          );

  const statusHint =
    userState === 'connected'
      ? t(
          'settings.system.extensionBridge.connectedHint',
          'The agent can use accounts you already signed into in Chrome. The built-in agent browser is still available. If the link drops, LibrAgent reconnects on its own.',
        )
      : userState === 'waiting_extension'
        ? t(
            'settings.system.extensionBridge.waitingHint',
            'Install the extension once and leave it on. Until then, the agent uses only the built-in agent browser.',
          )
        : null;

  return (
    <div className="space-y-4 max-w-lg">
      <div>
        <h4 className="text-sm font-medium text-foreground">
          {t('settings.system.extensionBridge.title', 'Your Chrome')}
        </h4>
        <p className="mt-1 text-xs text-muted-foreground leading-relaxed">
          {t(
            'settings.system.extensionBridge.description',
            'Optional. Install once so the agent can use sites where you are already signed in with Chrome. LibrAgent reconnects after you restart the app.',
          )}
        </p>
      </div>

      <div className="rounded-xl border border-border/70 bg-muted/20 p-4 space-y-2">
        <p className="text-sm font-medium text-foreground">
          <span
            className={
              connected
                ? 'text-emerald-600 dark:text-emerald-400'
                : 'text-foreground'
            }
          >
            {statusLabel}
          </span>
        </p>
        {statusHint ? (
          <p className="text-xs text-muted-foreground leading-relaxed">
            {statusHint}
          </p>
        ) : null}
        {!connected && !loading ? (
          <p className="text-xs text-muted-foreground leading-relaxed">
            {t(
              'settings.system.extensionBridge.fallbackNote',
              'Not connected yet. The agent uses the built-in agent browser only.',
            )}
          </p>
        ) : null}
      </div>

      {!connected && !loading ? (
        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            variant="default"
            className="h-8"
            onClick={() => void openStore()}
          >
            {t(
              'settings.system.extensionBridge.openStore',
              'Install from Chrome Web Store',
            )}
          </Button>
          <Button
            type="button"
            variant="outline"
            className="h-8"
            disabled={loading}
            onClick={() => void refresh()}
          >
            {t('settings.system.extensionBridge.refresh', 'Refresh status')}
          </Button>
        </div>
      ) : (
        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            variant="outline"
            className="h-8"
            disabled={loading}
            onClick={() => void refresh()}
          >
            {t('settings.system.extensionBridge.refresh', 'Refresh status')}
          </Button>
        </div>
      )}

      {!connected && !loading ? (
        <ol className="list-decimal list-inside space-y-1 text-xs text-muted-foreground leading-relaxed">
          <li>
            {t(
              'settings.system.extensionBridge.installStep1',
              'Install LibrAgent Browser Bridge from the Chrome Web Store (button above).',
            )}
          </li>
          <li>
            {t(
              'settings.system.extensionBridge.installStep2',
              'Open the extension icon and wait until the status shows Connected.',
            )}
          </li>
          <li>
            {t(
              'settings.system.extensionBridge.installStep3',
              'Leave the extension on. You do not need to open this page every time you restart LibrAgent.',
            )}
          </li>
        </ol>
      ) : null}

      {pathError ? (
        <p className="text-xs text-destructive">{pathError}</p>
      ) : null}

      <Collapsible open={advancedOpen} onOpenChange={setAdvancedOpen}>
        <CollapsibleTrigger asChild>
          <Button type="button" variant="ghost" className="h-8 px-2 text-xs">
            {advancedOpen
              ? t(
                  'settings.system.extensionBridge.hideAdvanced',
                  'Hide advanced',
                )
              : t(
                  'settings.system.extensionBridge.showAdvanced',
                  'Show advanced',
                )}
          </Button>
        </CollapsibleTrigger>
        <CollapsibleContent className="space-y-2 pt-2 text-xs text-muted-foreground">
          <p className="leading-relaxed">
            {t(
              'settings.system.extensionBridge.altInstallNote',
              'Developers can load the extension from a local folder instead of the Store. Copy the path below, then in Chrome open chrome://extensions, turn on Developer mode, and choose Load unpacked.',
            )}
          </p>
          {unpackedPath ? (
            <p className="break-all">
              <span className="font-medium text-foreground">
                {t('settings.system.extensionBridge.pathLabel', 'Local folder')}
                :{' '}
              </span>
              {unpackedPath}
            </p>
          ) : null}
          <Button
            type="button"
            variant="outline"
            className="h-8"
            disabled={!unpackedPath}
            onClick={() => void copyPath()}
          >
            {t(
              'settings.system.extensionBridge.copyPath',
              'Copy local folder path',
            )}
          </Button>
          {status ? (
            <>
              <p>
                <span className="font-medium text-foreground">
                  {t('settings.system.extensionBridge.portLabel', 'Port')}:{' '}
                </span>
                {status.port}
              </p>
              <p>
                <span className="font-medium text-foreground">
                  {t('settings.system.extensionBridge.tokenLabel', 'Token')}
                  :{' '}
                </span>
                {status.tokenHint}
              </p>
              <p>
                <span className="font-medium text-foreground">
                  {t(
                    'settings.system.extensionBridge.modeLabel',
                    'Connection mode',
                  )}
                  :{' '}
                </span>
                {status.backendMode}
              </p>
            </>
          ) : null}
          <p className="leading-relaxed">
            {t(
              'settings.system.extensionBridge.advancedNote',
              'Port and token are for support or local overrides. Most people only need the Store install and a running LibrAgent app.',
            )}
          </p>
        </CollapsibleContent>
      </Collapsible>
    </div>
  );
}
