import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
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
import {
  importBrowserProfiles,
  listBrowserProfiles,
  removeBrowserProfile,
  type BrowserProfileInfo,
} from '@/lib/backend/browser';
import { getLogger } from '@/lib/logger';

const logger = getLogger('BrowserProfilesSettings');

function formatImportedAt(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString();
}

export function BrowserProfilesSettingsSection() {
  const { t } = useTranslation('common');
  const [profiles, setProfiles] = useState<BrowserProfileInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [importing, setImporting] = useState(false);
  const [removingName, setRemovingName] = useState<string | null>(null);
  const [confirmImportOpen, setConfirmImportOpen] = useState(false);
  const [profilePendingRemoval, setProfilePendingRemoval] = useState<
    string | null
  >(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setErrorMessage(null);
    try {
      const next = await listBrowserProfiles();
      setProfiles(next);
    } catch (error) {
      logger.error('Failed to list browser profiles', error);
      setErrorMessage(
        t(
          'settings.system.browserProfiles.loadError',
          'Failed to load imported browser profiles.',
        ),
      );
    } finally {
      setLoading(false);
    }
  }, [t]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handleImport = async () => {
    setConfirmImportOpen(false);
    setImporting(true);
    setStatusMessage(null);
    setErrorMessage(null);
    try {
      const report = await importBrowserProfiles();
      await refresh();
      if (report.imported.length > 0) {
        setStatusMessage(
          t(
            'settings.system.browserProfiles.importSuccess',
            'Imported {{count}} profile(s). Close browsers before importing if cookie copy fails. Firefox imports cookies only (Chromium automation).',
            { count: report.imported.length },
          ),
        );
      } else if (report.warnings.length > 0) {
        setStatusMessage(report.warnings[0] ?? null);
      } else {
        setStatusMessage(
          t(
            'settings.system.browserProfiles.importNone',
            'No browser profiles were imported.',
          ),
        );
      }
      if (report.skipped.length > 0 && report.warnings.length > 0) {
        logger.warn('Browser profile import warnings', report.warnings);
      }
    } catch (error) {
      logger.error('Failed to import browser profiles', error);
      setErrorMessage(
        t(
          'settings.system.browserProfiles.importError',
          'Import failed. Close open browsers and try again.',
        ),
      );
    } finally {
      setImporting(false);
    }
  };

  const handleRemove = async (name: string) => {
    setProfilePendingRemoval(null);
    setRemovingName(name);
    setStatusMessage(null);
    setErrorMessage(null);
    try {
      await removeBrowserProfile(name);
      await refresh();
      setStatusMessage(
        t(
          'settings.system.browserProfiles.removeSuccess',
          'Removed imported profile "{{name}}".',
          { name },
        ),
      );
    } catch (error) {
      logger.error('Failed to remove browser profile', error);
      setErrorMessage(
        t(
          'settings.system.browserProfiles.removeError',
          'Failed to remove the imported profile.',
        ),
      );
    } finally {
      setRemovingName(null);
    }
  };

  return (
    <div className="rounded-xl border border-border/70 p-4 max-w-lg space-y-4">
      <div className="space-y-1">
        <h4 className="text-sm font-medium text-foreground">
          {t('settings.system.browserProfiles.title', 'Browser Profiles')}
        </h4>
        <p className="text-xs text-muted-foreground leading-relaxed">
          {t(
            'settings.system.browserProfiles.description',
            'Import installed browser Default profiles (Chrome, Edge, Brave, Chromium, Vivaldi, Firefox) into LibrAgent app storage. Chromium-family copies cookies and logins; Firefox imports cookies for Chromium automation. Agents can request them with use_profile=true after your confirmation. Default sessions stay clean and isolated.',
          )}
        </p>
      </div>

      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          variant="default"
          disabled={importing || removingName !== null}
          onClick={() => setConfirmImportOpen(true)}
        >
          {importing
            ? t('settings.system.browserProfiles.importing', 'Importing…')
            : t(
                'settings.system.browserProfiles.importButton',
                'Import browser profiles',
              )}
        </Button>
        <Button
          type="button"
          variant="outline"
          disabled={loading || importing || removingName !== null}
          onClick={() => void refresh()}
        >
          {t('settings.system.browserProfiles.refresh', 'Refresh')}
        </Button>
      </div>

      {statusMessage ? (
        <p className="text-xs text-muted-foreground">{statusMessage}</p>
      ) : null}
      {errorMessage ? (
        <p className="text-xs text-destructive">{errorMessage}</p>
      ) : null}

      {profiles.length > 0 ? (
        <ul className="space-y-2 text-sm">
          {profiles.map((profile) => (
            <li
              key={profile.name}
              className="flex items-center justify-between gap-3 rounded-lg bg-muted/40 px-3 py-2"
            >
              <div className="min-w-0">
                <p className="font-medium truncate">{profile.label}</p>
                <p className="text-xs text-muted-foreground truncate">
                  {profile.sourceBrowser} · {profile.sourceLabel}
                  {profile.isDefault
                    ? ` · ${t('settings.system.browserProfiles.defaultBadge', 'default')}`
                    : ''}
                </p>
                <p className="text-xs text-muted-foreground truncate">
                  {t('settings.system.browserProfiles.importedAt', 'Imported')}:{' '}
                  {formatImportedAt(profile.importedAt)}
                </p>
              </div>
              <div className="flex shrink-0 items-center gap-2">
                <span className="font-mono text-xs text-muted-foreground">
                  {profile.name}
                </span>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="h-7 px-2 text-xs text-destructive hover:text-destructive"
                  disabled={importing || removingName !== null}
                  onClick={() => setProfilePendingRemoval(profile.name)}
                >
                  {removingName === profile.name
                    ? t('settings.system.browserProfiles.removing', 'Removing…')
                    : t('settings.system.browserProfiles.remove', 'Remove')}
                </Button>
              </div>
            </li>
          ))}
        </ul>
      ) : (
        !loading && (
          <p className="text-xs text-muted-foreground">
            {t(
              'settings.system.browserProfiles.empty',
              'No profiles imported yet.',
            )}
          </p>
        )
      )}

      <AlertDialog open={confirmImportOpen} onOpenChange={setConfirmImportOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t(
                'settings.system.browserProfiles.importConfirmTitle',
                'Import browser profiles?',
              )}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t(
                'settings.system.browserProfiles.importConfirm',
                'This copies Default profiles from installed browsers (Chrome/Edge/Brave/Firefox, …) into LibrAgent app storage. Close those browsers first for a complete copy. Firefox contributes cookies only (automation still uses Chromium). You can remove imported copies later from this screen.',
              )}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('common.cancel', 'Cancel')}</AlertDialogCancel>
            <AlertDialogAction onClick={() => void handleImport()}>
              {t(
                'settings.system.browserProfiles.importConfirmAction',
                'Import',
              )}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <AlertDialog
        open={profilePendingRemoval !== null}
        onOpenChange={(open) => {
          if (!open) {
            setProfilePendingRemoval(null);
          }
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t(
                'settings.system.browserProfiles.removeConfirmTitle',
                'Remove imported profile?',
              )}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t(
                'settings.system.browserProfiles.removeConfirm',
                'Delete the LibrAgent copy of "{{name}}" (cookies/logins in app storage)? Your real browser profile is not modified.',
                { name: profilePendingRemoval ?? '' },
              )}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('common.cancel', 'Cancel')}</AlertDialogCancel>
            <AlertDialogAction
              className="bg-destructive text-white hover:bg-destructive/90"
              onClick={() => {
                if (profilePendingRemoval) {
                  void handleRemove(profilePendingRemoval);
                }
              }}
            >
              {t('settings.system.browserProfiles.remove', 'Remove')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
