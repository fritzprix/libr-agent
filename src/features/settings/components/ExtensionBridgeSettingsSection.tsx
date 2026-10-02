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

type UserFacingState = 'checking' | 'connected' | 'waiting_extension';

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
              'Could not read extension bridge status.',
            );
      toast.error(
        t(
          'settings.system.extensionBridge.statusErrorTitle',
          'Bridge status unavailable',
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
              'Could not resolve the Load unpacked path.',
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
          'Install path copied',
        ),
      );
    } catch {
      toast.error(
        t(
          'settings.system.extensionBridge.pathCopyFailed',
          'Could not copy path to clipboard',
        ),
      );
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
            'Connected to everyday Chrome',
          )
        : t(
            'settings.system.extensionBridge.waitingHeadline',
            'Waiting for Chrome extension',
          );

  const statusHint =
    userState === 'connected'
      ? t(
          'settings.system.extensionBridge.connectedHint',
          'Agents prefer your logged-in Chrome tabs. If the link drops, LibrAgent reconnects automatically when this app is running — you should not need to reopen chrome://extensions.',
        )
      : userState === 'waiting_extension'
        ? t(
            'settings.system.extensionBridge.waitingHint',
            'One-time setup: install the Load unpacked extension, then leave it enabled. While disconnected, agents use the built-in agent browser below as a fallback.',
          )
        : null;

  return (
    <div className="space-y-4 max-w-lg">
      <div>
        <h4 className="text-sm font-medium text-foreground">
          {t(
            'settings.system.extensionBridge.title',
            'Everyday Chrome',
          )}
        </h4>
        <p className="mt-1 text-xs text-muted-foreground leading-relaxed">
          {t(
            'settings.system.extensionBridge.description',
            'Optional. Connect once so agents can browse with the logins already in your everyday Chrome. Restarting LibrAgent should reconnect by itself.',
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
              'Fallback is on: sticky agent browser (separate from everyday Chrome).',
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
            disabled={!unpackedPath}
            onClick={() => void copyPath()}
          >
            {t(
              'settings.system.extensionBridge.copyPath',
              'Copy install path',
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
              'Chrome → chrome://extensions → enable Developer mode',
            )}
          </li>
          <li>
            {t(
              'settings.system.extensionBridge.installStep2',
              'Load unpacked → paste the install path (button above)',
            )}
          </li>
          <li>
            {t(
              'settings.system.extensionBridge.installStep3',
              'Leave the extension enabled. Click its icon later to see connection status — do not reopen this page on every app restart.',
            )}
          </li>
        </ol>
      ) : null}

      {pathError ? <p className="text-xs text-destructive">{pathError}</p> : null}

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
                  {t('settings.system.extensionBridge.tokenLabel', 'Token')}:{' '}
                </span>
                {status.tokenHint}
              </p>
              <p>
                <span className="font-medium text-foreground">
                  {t('settings.system.extensionBridge.modeLabel', 'Backend')}:{' '}
                </span>
                {status.backendMode}
              </p>
            </>
          ) : null}
          {unpackedPath ? (
            <p className="break-all">
              <span className="font-medium text-foreground">
                {t(
                  'settings.system.extensionBridge.pathLabel',
                  'Install path',
                )}
                :{' '}
              </span>
              {unpackedPath}
            </p>
          ) : null}
          <p className="leading-relaxed">
            {t(
              'settings.system.extensionBridge.advancedNote',
              'Port/token are for support and local overrides. Everyday use only needs the extension installed once and LibrAgent running.',
            )}
          </p>
          {connected ? (
            <Button
              type="button"
              variant="outline"
              className="h-8"
              disabled={!unpackedPath}
              onClick={() => void copyPath()}
            >
              {t(
                'settings.system.extensionBridge.copyPath',
                'Copy install path',
              )}
            </Button>
          ) : null}
        </CollapsibleContent>
      </Collapsible>
    </div>
  );
}
