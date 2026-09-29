import { useCallback, useEffect, useMemo, useState } from 'react';
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
import { Checkbox } from '@/components/ui/checkbox';
import {
  checkBrowserProfileImportReady,
  importBrowserProfiles,
  listBrowserProfiles,
  listDiscoverableBrowserProfiles,
  quitBrowsersForProfileImport,
  removeBrowserProfile,
  setDefaultBrowserProfile,
  openBrowserProfileForSignIn,
  type BrowserProfileInfo,
  type DiscoverableBrowserProfile,
} from '@/lib/backend/browser';
import { getLogger } from '@/lib/logger';

const logger = getLogger('BrowserProfilesSettings');

type DialogMode = 'closed' | 'close-browsers' | 'confirm' | 'remove';

function formatImportedAt(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString();
}

function formatBrowserList(names: string[]): string {
  return names.join(', ');
}

export function BrowserProfilesSettingsSection() {
  const { t } = useTranslation('common');
  const [profiles, setProfiles] = useState<BrowserProfileInfo[]>([]);
  const [discoverable, setDiscoverable] = useState<DiscoverableBrowserProfile[]>(
    [],
  );
  const [selectedNames, setSelectedNames] = useState<string[]>([]);
  const [preferredDefault, setPreferredDefault] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [importing, setImporting] = useState(false);
  const [checkingReady, setCheckingReady] = useState(false);
  const [quitting, setQuitting] = useState(false);
  const [settingDefault, setSettingDefault] = useState<string | null>(null);
  const [removingName, setRemovingName] = useState<string | null>(null);
  const [signingInName, setSigningInName] = useState<string | null>(null);
  const [dialogMode, setDialogMode] = useState<DialogMode>('closed');
  const [runningBrowsers, setRunningBrowsers] = useState<string[]>([]);
  const [profilePendingRemoval, setProfilePendingRemoval] = useState<{
    name: string;
    label: string;
  } | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [statusTone, setStatusTone] = useState<'ok' | 'warn' | 'error'>('ok');
  const [blockerVisible, setBlockerVisible] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [next, available] = await Promise.all([
        listBrowserProfiles(),
        listDiscoverableBrowserProfiles(),
      ]);
      setProfiles(next);
      setDiscoverable(available);
      setSelectedNames((prev) => {
        const availableNames = new Set(available.map((p) => p.name));
        const kept = prev.filter((name) => availableNames.has(name));
        if (kept.length > 0) {
          return kept;
        }
        // Prefer system-priority first discoverable (Chrome > Edge > …) when nothing selected.
        return available.length > 0 ? [available[0].name] : [];
      });
      setPreferredDefault((prev) => {
        if (prev && available.some((p) => p.name === prev)) {
          return prev;
        }
        return available.length > 0 ? available[0].name : null;
      });
    } catch (error) {
      logger.error('Failed to list browser profiles', error);
      setStatusTone('error');
      setStatusMessage(
        t(
          'settings.system.browserProfiles.loadError',
          'Could not load your saved browser logins. Try again in a moment.',
        ),
      );
    } finally {
      setLoading(false);
    }
  }, [t]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const selectedProfiles = useMemo(
    () => discoverable.filter((p) => selectedNames.includes(p.name)),
    [discoverable, selectedNames],
  );

  const refreshReadiness = useCallback(async (): Promise<boolean> => {
    setCheckingReady(true);
    try {
      const readiness = await checkBrowserProfileImportReady(
        selectedNames.length > 0 ? selectedNames : undefined,
      );
      setRunningBrowsers(readiness.runningBrowsers);
      return readiness.ready;
    } catch (error) {
      logger.error('Failed to check browser import readiness', error);
      setRunningBrowsers([]);
      return true;
    } finally {
      setCheckingReady(false);
    }
  }, [selectedNames]);

  const toggleSelected = (name: string, checked: boolean) => {
    setSelectedNames((prev) => {
      if (checked) {
        return prev.includes(name) ? prev : [...prev, name];
      }
      return prev.filter((n) => n !== name);
    });
    if (checked) {
      // Prefer the newly checked browser when nothing is preferred yet,
      // or when the previous preferred was unchecked.
      setPreferredDefault((prev) => {
        if (prev === null || prev === name) {
          return name;
        }
        if (!selectedNames.includes(prev)) {
          return name;
        }
        return prev;
      });
    }
    if (!checked && preferredDefault === name) {
      const remaining = selectedNames.filter((n) => n !== name);
      setPreferredDefault(remaining[0] ?? null);
    }
  };

  const runImport = async () => {
    setDialogMode('closed');
    setImporting(true);
    setStatusMessage(null);
    setBlockerVisible(false);
    try {
      const names =
        selectedNames.length > 0 ? selectedNames : undefined;
      const preferred =
        preferredDefault && names?.includes(preferredDefault)
          ? preferredDefault
          : names?.[0];
      const report = await importBrowserProfiles({
        profileNames: names,
        preferredDefault: preferred,
      });
      await refresh();
      const stillOpen = report.runningBrowsers ?? [];
      setRunningBrowsers(stillOpen);

      if (report.imported.length > 0 && report.skipped.length === 0) {
        setStatusTone('ok');
        setStatusMessage(
          t(
            'settings.system.browserProfiles.importSuccess',
            'Done — your browser logins are ready for LibrAgent. When an agent needs them, you will be asked to confirm first.',
          ),
        );
        return;
      }

      if (report.imported.length > 0 && report.skipped.length > 0) {
        setStatusTone('warn');
        setStatusMessage(
          t(
            'settings.system.browserProfiles.importPartial',
            'Some logins were saved, but others could not be copied because a browser was still open.',
          ),
        );
        if (stillOpen.length > 0) {
          setBlockerVisible(true);
        }
        return;
      }

      if (stillOpen.length > 0 || report.skipped.length > 0) {
        setStatusTone('warn');
        setStatusMessage(
          t(
            'settings.system.browserProfiles.importBlocked',
            'Import could not finish because a browser is still open. Tap Quit browsers, then try again.',
          ),
        );
        setBlockerVisible(true);
        return;
      }

      if (report.warnings.length > 0) {
        setStatusTone('warn');
        setStatusMessage(
          t(
            'settings.system.browserProfiles.importNoneFound',
            'No supported browser profiles were found. Install Chrome, Edge, or Brave and sign in there first.',
          ),
        );
        return;
      }

      setStatusTone('warn');
      setStatusMessage(
        t(
          'settings.system.browserProfiles.importNone',
          'Nothing was imported. Make sure a browser is installed and you have signed in at least once.',
        ),
      );
    } catch (error) {
      logger.error('Failed to import browser profiles', error);
      const ready = await refreshReadiness();
      setStatusTone('error');
      if (!ready) {
        setStatusMessage(
          t(
            'settings.system.browserProfiles.importBlocked',
            'Import could not finish because a browser is still open. Tap Quit browsers, then try again.',
          ),
        );
        setBlockerVisible(true);
      } else {
        setStatusMessage(
          t(
            'settings.system.browserProfiles.importError',
            'Import failed. Quit the browsers from this screen, wait a few seconds, then try again.',
          ),
        );
      }
    } finally {
      setImporting(false);
    }
  };

  const handleImportClick = async () => {
    setStatusMessage(null);
    setBlockerVisible(false);
    if (selectedNames.length === 0) {
      setStatusTone('warn');
      setStatusMessage(
        t(
          'settings.system.browserProfiles.selectRequired',
          'Choose at least one browser to import.',
        ),
      );
      return;
    }
    const ready = await refreshReadiness();
    if (!ready) {
      setDialogMode('close-browsers');
      return;
    }
    setDialogMode('confirm');
  };

  const handleQuitBrowsers = async () => {
    setQuitting(true);
    setStatusMessage(null);
    try {
      const report = await quitBrowsersForProfileImport(
        true,
        selectedNames.length > 0 ? selectedNames : undefined,
      );
      setRunningBrowsers(report.stillRunning);
      if (report.ready) {
        setBlockerVisible(false);
        setDialogMode('confirm');
        setStatusTone('ok');
        setStatusMessage(
          t(
            'settings.system.browserProfiles.quitSuccess',
            'Browsers were closed. You can import your logins now.',
          ),
        );
        return;
      }
      setBlockerVisible(true);
      setDialogMode('close-browsers');
      setStatusTone('warn');
      setStatusMessage(
        t(
          'settings.system.browserProfiles.quitPartial',
          'Still open: {{browsers}}. Tap Quit browsers again, or close any remaining windows.',
          { browsers: formatBrowserList(report.stillRunning) },
        ),
      );
    } catch (error) {
      logger.error('Failed to quit browsers for import', error);
      setStatusTone('error');
      setStatusMessage(
        t(
          'settings.system.browserProfiles.quitError',
          'Could not close the browsers automatically. Close them yourself, then try again.',
        ),
      );
    } finally {
      setQuitting(false);
    }
  };

  const handleSetDefault = async (name: string) => {
    setSettingDefault(name);
    setStatusMessage(null);
    try {
      await setDefaultBrowserProfile(name);
      await refresh();
      setStatusTone('ok');
      setStatusMessage(
        t(
          'settings.system.browserProfiles.setDefaultSuccess',
          'Primary login updated. Agents will use this profile when you approve.',
        ),
      );
    } catch (error) {
      logger.error('Failed to set default browser profile', error);
      setStatusTone('error');
      setStatusMessage(
        t(
          'settings.system.browserProfiles.setDefaultError',
          'Could not update the primary login. Try again in a moment.',
        ),
      );
    } finally {
      setSettingDefault(null);
    }
  };

  const handleRemove = async (name: string) => {
    setDialogMode('closed');
    setProfilePendingRemoval(null);
    setRemovingName(name);
    setStatusMessage(null);
    setBlockerVisible(false);
    try {
      await removeBrowserProfile(name);
      await refresh();
      setStatusTone('ok');
      setStatusMessage(
        t(
          'settings.system.browserProfiles.removeSuccess',
          'Removed. Your real browser is unchanged — only the LibrAgent copy was deleted.',
        ),
      );
    } catch (error) {
      logger.error('Failed to remove browser profile', error);
      setStatusTone('error');
      setStatusMessage(
        t(
          'settings.system.browserProfiles.removeError',
          'Could not remove that copy. Try again in a moment.',
        ),
      );
    } finally {
      setRemovingName(null);
    }
  };

  const handleOpenForSignIn = async (name: string) => {
    setSigningInName(name);
    setStatusMessage(null);
    try {
      await openBrowserProfileForSignIn(name);
      setStatusTone('ok');
      setStatusMessage(
        t(
          'settings.system.browserProfiles.signInOpened',
          'Chrome opened for this LibrAgent login. Sign in to Google (and any other sites) in that window, then close it. Agents can reuse those logins after you approve.',
        ),
      );
    } catch (error) {
      logger.error('Failed to open browser profile for sign-in', error);
      setStatusTone('error');
      const message =
        error instanceof Error
          ? error.message
          : t(
              'settings.system.browserProfiles.signInError',
              'Could not open Chrome for sign-in. Install Google Chrome or Microsoft Edge and try again.',
            );
      setStatusMessage(message);
    } finally {
      setSigningInName(null);
    }
  };

  const busy =
    importing ||
    removingName !== null ||
    checkingReady ||
    quitting ||
    settingDefault !== null ||
    signingInName !== null;

  const selectedBrowserLabels = selectedProfiles.map((p) => p.browserLabel);
  const confirmBrowserList =
    selectedBrowserLabels.length > 0
      ? formatBrowserList([...new Set(selectedBrowserLabels)])
      : '';

  return (
    <div className="rounded-xl border border-border/70 p-4 max-w-lg space-y-4">
      <div className="space-y-1">
        <h4 className="text-sm font-medium text-foreground">
          {t('settings.system.browserProfiles.title', 'Saved browser logins')}
        </h4>
        <p className="text-xs text-muted-foreground leading-relaxed">
          {t(
            'settings.system.browserProfiles.description',
            'Copies Chrome/Edge/Brave into a LibrAgent-only profile (not your everyday browser). Import may reuse some sessions; for Google, use Open to sign in if needed. Agents use this copy only after you approve.',
          )}
        </p>
      </div>

      {discoverable.length > 0 ? (
        <div className="space-y-2">
          <p className="text-xs font-medium text-foreground">
            {t(
              'settings.system.browserProfiles.selectTitle',
              'Browsers to import',
            )}
          </p>
          <p className="text-xs text-muted-foreground">
            {t(
              'settings.system.browserProfiles.selectHint',
              'Only selected browsers are checked and closed. Importing Edge will not ask you to quit Chrome.',
            )}
          </p>
          <ul className="space-y-2">
            {discoverable.map((profile) => {
              const checked = selectedNames.includes(profile.name);
              return (
                <li
                  key={profile.name}
                  className="flex items-start gap-3 rounded-lg bg-muted/40 px-3 py-2"
                >
                  <Checkbox
                    id={`import-${profile.name}`}
                    checked={checked}
                    disabled={busy}
                    onCheckedChange={(value) => {
                      toggleSelected(profile.name, value === true);
                    }}
                    className="mt-0.5"
                  />
                  <div className="min-w-0 flex-1 space-y-1">
                    <label
                      htmlFor={`import-${profile.name}`}
                      className="text-sm font-medium cursor-pointer"
                    >
                      {profile.browserLabel}
                    </label>
                    <p className="text-xs text-muted-foreground truncate">
                      {profile.label}
                    </p>
                    {checked ? (
                      <label className="flex items-center gap-2 text-xs text-muted-foreground cursor-pointer">
                        <input
                          type="radio"
                          name="preferred-default"
                          className="h-3.5 w-3.5 accent-primary"
                          checked={preferredDefault === profile.name}
                          disabled={busy}
                          onChange={() => setPreferredDefault(profile.name)}
                        />
                        {t(
                          'settings.system.browserProfiles.preferAsPrimary',
                          'Use as primary after import',
                        )}
                      </label>
                    ) : null}
                  </div>
                </li>
              );
            })}
          </ul>
        </div>
      ) : (
        !loading && (
          <p className="text-xs text-muted-foreground">
            {t(
              'settings.system.browserProfiles.noneDiscoverable',
              'No supported browsers found on this computer yet.',
            )}
          </p>
        )
      )}

      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          variant="default"
          disabled={busy || selectedNames.length === 0}
          onClick={() => void handleImportClick()}
        >
          {importing
            ? t('settings.system.browserProfiles.importing', 'Importing…')
            : checkingReady
              ? t('settings.system.browserProfiles.checking', 'Checking…')
              : t(
                  'settings.system.browserProfiles.importButton',
                  'Import selected',
                )}
        </Button>
        <Button
          type="button"
          variant="outline"
          disabled={loading || busy}
          onClick={() => void refresh()}
        >
          {t('settings.system.browserProfiles.refresh', 'Refresh list')}
        </Button>
      </div>

      {statusMessage ? (
        <p
          className={
            statusTone === 'error'
              ? 'text-xs text-destructive'
              : statusTone === 'warn'
                ? 'text-xs text-amber-700 dark:text-amber-400'
                : 'text-xs text-muted-foreground'
          }
        >
          {statusMessage}
        </p>
      ) : null}

      {blockerVisible && runningBrowsers.length > 0 ? (
        <div className="rounded-lg border border-amber-500/40 bg-amber-500/10 p-3 space-y-3">
          <p className="text-xs font-medium text-foreground">
            {t(
              'settings.system.browserProfiles.closeNeededTitle',
              'Browsers are still open',
            )}
          </p>
          <p className="text-xs text-muted-foreground">
            {t(
              'settings.system.browserProfiles.closeNeededBody',
              'Still open: {{browsers}}. LibrAgent can close them for you (unsaved tabs may be lost).',
              { browsers: formatBrowserList(runningBrowsers) },
            )}
          </p>
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              size="sm"
              disabled={busy}
              onClick={() => void handleQuitBrowsers()}
            >
              {quitting
                ? t(
                    'settings.system.browserProfiles.quitting',
                    'Closing browsers…',
                  )
                : t(
                    'settings.system.browserProfiles.quitBrowsers',
                    'Quit browsers for me',
                  )}
            </Button>
          </div>
        </div>
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
                  {profile.sourceBrowser}
                  {profile.isDefault
                    ? ` · ${t('settings.system.browserProfiles.defaultBadge', 'primary')}`
                    : ''}
                </p>
                <p className="text-xs text-muted-foreground truncate">
                  {t('settings.system.browserProfiles.importedAt', 'Saved')}:{' '}
                  {formatImportedAt(profile.importedAt)}
                </p>
              </div>
              <div className="flex shrink-0 items-center gap-1">
                {profile.importKind !== 'firefox_cookies' ? (
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-7 px-2 text-xs"
                    disabled={busy}
                    onClick={() => void handleOpenForSignIn(profile.name)}
                  >
                    {signingInName === profile.name
                      ? t(
                          'settings.system.browserProfiles.signingIn',
                          'Opening…',
                        )
                      : t(
                          'settings.system.browserProfiles.openToSignIn',
                          'Open to sign in',
                        )}
                  </Button>
                ) : null}
                {!profile.isDefault ? (
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-7 px-2 text-xs"
                    disabled={busy}
                    onClick={() => void handleSetDefault(profile.name)}
                  >
                    {settingDefault === profile.name
                      ? t(
                          'settings.system.browserProfiles.settingDefault',
                          'Updating…',
                        )
                      : t(
                          'settings.system.browserProfiles.setAsPrimary',
                          'Set primary',
                        )}
                  </Button>
                ) : null}
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="h-7 px-2 text-xs text-destructive hover:text-destructive"
                  disabled={busy}
                  onClick={() => {
                    setProfilePendingRemoval({
                      name: profile.name,
                      label: profile.label,
                    });
                    setDialogMode('remove');
                  }}
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
              'Nothing saved yet. Import from your browsers when you are ready.',
            )}
          </p>
        )
      )}

      <AlertDialog
        open={dialogMode === 'close-browsers'}
        onOpenChange={(open) => {
          if (!open && !quitting) {
            // Stay on confirm/remove if we already advanced; don't clobber after quit success.
            setDialogMode((mode) =>
              mode === 'close-browsers' ? 'closed' : mode,
            );
          }
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t(
                'settings.system.browserProfiles.closeDialogTitle',
                'Quit browsers to continue?',
              )}
            </AlertDialogTitle>
            <AlertDialogDescription asChild>
              <div className="space-y-3 text-sm text-muted-foreground">
                <p>
                  {t(
                    'settings.system.browserProfiles.closeDialogBody',
                    'To copy your logins, these apps must be closed: {{browsers}}. LibrAgent can quit them for you now.',
                    {
                      browsers:
                        formatBrowserList(runningBrowsers) ||
                        t(
                          'settings.system.browserProfiles.closeDialogBrowsersFallback',
                          'your browser',
                        ),
                    },
                  )}
                </p>
                <p>
                  {t(
                    'settings.system.browserProfiles.quitConsentWarning',
                    'Unsaved work in open tabs may be lost. Save anything important first.',
                  )}
                </p>
              </div>
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={quitting}>
              {t('common.cancel', 'Cancel')}
            </AlertDialogCancel>
            <AlertDialogAction
              disabled={quitting}
              onClick={(event) => {
                event.preventDefault();
                void handleQuitBrowsers();
              }}
            >
              {quitting
                ? t(
                    'settings.system.browserProfiles.quitting',
                    'Closing browsers…',
                  )
                : t(
                    'settings.system.browserProfiles.quitBrowsers',
                    'Quit browsers for me',
                  )}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <AlertDialog
        open={dialogMode === 'confirm'}
        onOpenChange={(open) => {
          if (!open) {
            setDialogMode('closed');
          }
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t(
                'settings.system.browserProfiles.importConfirmTitle',
                'Import your browser logins?',
              )}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {confirmBrowserList
                ? t(
                    'settings.system.browserProfiles.importConfirmSelected',
                    'LibrAgent will copy signed-in sessions from {{browsers}} into its own private folder. Your original browsers are not changed. Agents can use these logins only after you approve.',
                    { browsers: confirmBrowserList },
                  )
                : t(
                    'settings.system.browserProfiles.importConfirm',
                    'LibrAgent will copy signed-in sessions from browsers on this computer into its own private folder. Your original browsers are not changed. Agents can use these logins only after you approve.',
                  )}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('common.cancel', 'Cancel')}</AlertDialogCancel>
            <AlertDialogAction onClick={() => void runImport()}>
              {t(
                'settings.system.browserProfiles.importConfirmAction',
                'Import',
              )}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <AlertDialog
        open={dialogMode === 'remove'}
        onOpenChange={(open) => {
          if (!open) {
            setDialogMode('closed');
            setProfilePendingRemoval(null);
          }
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t(
                'settings.system.browserProfiles.removeConfirmTitle',
                'Remove this saved copy?',
              )}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t(
                'settings.system.browserProfiles.removeConfirm',
                'Delete LibrAgent’s copy of “{{name}}”? Your real browser and its passwords stay untouched.',
                { name: profilePendingRemoval?.label ?? '' },
              )}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('common.cancel', 'Cancel')}</AlertDialogCancel>
            <AlertDialogAction
              className="bg-destructive text-white hover:bg-destructive/90"
              onClick={() => {
                if (profilePendingRemoval) {
                  void handleRemove(profilePendingRemoval.name);
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
