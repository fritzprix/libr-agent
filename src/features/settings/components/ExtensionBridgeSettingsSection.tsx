import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { Button } from '@/components/ui';
import {
  getExtensionBridgeStatus,
  getExtensionUnpackedPath,
  type ExtensionBridgeStatus,
} from '@/lib/backend/browser';

export function ExtensionBridgeSettingsSection() {
  const { t } = useTranslation('common');
  const [status, setStatus] = useState<ExtensionBridgeStatus | null>(null);
  const [unpackedPath, setUnpackedPath] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [pathError, setPathError] = useState<string | null>(null);

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
          'Load unpacked path copied',
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
  const statusLabel = loading
    ? t('settings.system.extensionBridge.checking', 'Checking…')
    : connected
      ? t('settings.system.extensionBridge.connected', 'Connected')
      : t('settings.system.extensionBridge.disconnected', 'Disconnected');

  return (
    <div className="space-y-4 max-w-lg">
      <div>
        <h4 className="text-sm font-medium text-foreground">
          {t(
            'settings.system.extensionBridge.title',
            'Chrome extension bridge',
          )}
        </h4>
        <p className="mt-1 text-xs text-muted-foreground leading-relaxed">
          {t(
            'settings.system.extensionBridge.description',
            'Optional Load unpacked MV3 extension for everyday Chrome. When connected, create/navigate/close prefer the extension; otherwise LibrAgent uses the sticky agent browser sidecar.',
          )}
        </p>
      </div>

      <div className="space-y-2 text-xs text-muted-foreground">
        <p>
          <span className="font-medium text-foreground">
            {t('settings.system.extensionBridge.statusLabel', 'Status')}:{' '}
          </span>
          <span
            className={
              connected ? 'text-emerald-600 dark:text-emerald-400' : undefined
            }
          >
            {statusLabel}
          </span>
        </p>
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
                'Load unpacked path',
              )}
              :{' '}
            </span>
            {unpackedPath}
          </p>
        ) : null}
        {pathError ? <p className="text-destructive">{pathError}</p> : null}
      </div>

      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          variant="outline"
          className="h-8"
          disabled={loading}
          onClick={() => void refresh()}
        >
          {t('settings.system.extensionBridge.refresh', 'Refresh')}
        </Button>
        <Button
          type="button"
          variant="outline"
          className="h-8"
          disabled={!unpackedPath}
          onClick={() => void copyPath()}
        >
          {t(
            'settings.system.extensionBridge.copyPath',
            'Copy Load unpacked path',
          )}
        </Button>
      </div>
    </div>
  );
}
