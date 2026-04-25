import { useTranslation } from 'react-i18next';
import {
  RefreshCw,
  CheckCircle,
  AlertCircle,
  Loader2,
  Download,
} from 'lucide-react';
import { Button } from '@/components/ui';
import { useUpdateContext } from '@/context/UpdateContext';

/**
 * Displays the current app version and provides a manual "Check for Updates"
 * button. Consumes UpdateContext — no props required.
 */
export function AboutSection() {
  const { t } = useTranslation('common');
  const {
    status,
    availableVersion,
    downloadProgress,
    error,
    canInstallUpdate,
    installHint,
    checkForUpdate,
    installUpdate,
  } = useUpdateContext();

  const isChecking = status === 'checking';
  const isDownloading = status === 'downloading' || status === 'installing';

  return (
    <div className="space-y-3">
      <h3 className="font-semibold text-foreground">
        {t('settings.about.title', 'About')}
      </h3>

      <div className="rounded-lg border bg-card p-4 space-y-4">
        {/* Version row */}
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm font-medium text-foreground">
              {t('appName', 'LibrAgent')}
            </p>
            <p className="text-xs text-muted-foreground">
              {t('settings.about.version', 'Version')} {__APP_VERSION__}
            </p>
            {!canInstallUpdate && installHint && (
              <p className="mt-1 max-w-xl text-xs text-muted-foreground">
                {installHint}
              </p>
            )}
          </div>

          {/* Status indicator */}
          <div className="flex items-center gap-2">
            {status === 'available' && availableVersion && (
              <span className="text-xs font-medium text-primary">
                {t('settings.about.updateAvailable', 'v{{version}} available', {
                  version: availableVersion,
                })}
              </span>
            )}
            {status === 'up-to-date' && (
              <span className="flex items-center gap-1 text-xs text-muted-foreground">
                <CheckCircle size={12} className="text-green-500" />
                {t('settings.about.upToDate', 'Up to date')}
              </span>
            )}
            {status === 'error' && (
              <span
                className="flex items-center gap-1 text-xs text-destructive"
                title={error || undefined}
              >
                <AlertCircle size={12} />
                {t('settings.about.checkFailed', 'Check failed')}
              </span>
            )}
            {isDownloading && (
              <span className="flex items-center gap-1 text-xs text-muted-foreground">
                <Loader2 size={12} className="animate-spin" />
                {status === 'installing'
                  ? t('settings.about.installing', 'Installing…')
                  : t('settings.about.downloading', 'Downloading {{pct}}%', {
                      pct: downloadProgress,
                    })}
              </span>
            )}
          </div>
        </div>

        {/* Action buttons */}
        <div className="flex gap-2">
          {status === 'available' ? (
            <Button
              size="sm"
              onClick={() => void installUpdate()}
              disabled={isDownloading || !canInstallUpdate}
              className="gap-1.5"
              title={!canInstallUpdate ? installHint || undefined : undefined}
            >
              <Download size={14} />
              {t('settings.about.installUpdate', 'Install Update')}
            </Button>
          ) : (
            <Button
              variant="outline"
              size="sm"
              onClick={() => void checkForUpdate()}
              disabled={isChecking || isDownloading}
              className="gap-1.5"
            >
              <RefreshCw
                size={14}
                className={isChecking ? 'animate-spin' : ''}
              />
              {t('settings.about.checkForUpdates', 'Check for Updates')}
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
