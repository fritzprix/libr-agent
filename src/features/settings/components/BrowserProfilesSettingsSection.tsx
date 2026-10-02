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
  quitBrowserProfileSignInWindows,
  type BrowserProfileInfo,
  type DiscoverableBrowserProfile,
} from '@/lib/backend/browser';
import { getLogger } from '@/lib/logger';

const logger = getLogger('BrowserProfilesSettings');

/** Honest import flow — never pretend this is one click. */
type WizardStep = 'select' | 'close' | 'copy' | 'done';

type ImportOutcome = 'success' | 'partial' | 'blocked' | 'none' | 'error';

const WIZARD_STEPS: WizardStep[] = ['select', 'close', 'copy', 'done'];

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

function WizardStepList({
  active,
  labels,
}: {
  active: WizardStep;
  labels: Record<WizardStep, string>;
}) {
  const activeIndex = WIZARD_STEPS.indexOf(active);
  return (
    <ol className="flex flex-wrap gap-x-3 gap-y-1 text-xs">
      {WIZARD_STEPS.map((step, index) => {
        const isActive = step === active;
        const isDone = index < activeIndex;
        return (
          <li
            key={step}
            className={
              isActive
                ? 'font-medium text-foreground'
                : isDone
                  ? 'text-muted-foreground'
                  : 'text-muted-foreground/60'
            }
          >
            <span className="tabular-nums">{index + 1}.</span> {labels[step]}
          </li>
        );
      })}
    </ol>
  );
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
  const [closingSignInName, setClosingSignInName] = useState<string | null>(null);
  const [signInHelpOpenFor, setSignInHelpOpenFor] = useState<string | null>(
    null,
  );
  const [signInSessionFor, setSignInSessionFor] = useState<string | null>(null);
  const [removeDialogOpen, setRemoveDialogOpen] = useState(false);
  const [runningBrowsers, setRunningBrowsers] = useState<string[]>([]);
  const [browsersReady, setBrowsersReady] = useState(false);
  const [profilePendingRemoval, setProfilePendingRemoval] = useState<{
    name: string;
    label: string;
  } | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [statusTone, setStatusTone] = useState<'ok' | 'warn' | 'error'>('ok');
  const [wizardOpen, setWizardOpen] = useState(false);
  const [wizardStep, setWizardStep] = useState<WizardStep>('select');
  const [importOutcome, setImportOutcome] = useState<ImportOutcome | null>(
    null,
  );

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

  const selectedBrowserLabels = selectedProfiles.map((p) => p.browserLabel);
  const confirmBrowserList =
    selectedBrowserLabels.length > 0
      ? formatBrowserList([...new Set(selectedBrowserLabels)])
      : '';

  const wizardStepLabels = useMemo(
    () =>
      ({
        select: t(
          'settings.system.browserProfiles.wizardStepSelect',
          'Choose',
        ),
        close: t('settings.system.browserProfiles.wizardStepClose', 'Close'),
        copy: t('settings.system.browserProfiles.wizardStepCopy', 'Copy'),
        done: t('settings.system.browserProfiles.wizardStepDone', 'Done'),
      }) satisfies Record<WizardStep, string>,
    [t],
  );

  const refreshReadiness = useCallback(async (): Promise<boolean> => {
    setCheckingReady(true);
    try {
      const readiness = await checkBrowserProfileImportReady(
        selectedNames.length > 0 ? selectedNames : undefined,
      );
      setRunningBrowsers(readiness.runningBrowsers);
      setBrowsersReady(readiness.ready);
      return readiness.ready;
    } catch (error) {
      logger.error('Failed to check browser import readiness', error);
      setRunningBrowsers([]);
      setBrowsersReady(false);
      return false;
    } finally {
      setCheckingReady(false);
    }
  }, [selectedNames]);

  const toggleSelected = (name: string, checked: boolean) => {
    setSelectedNames((prevSelected) => {
      const nextSelected = checked
        ? prevSelected.includes(name)
          ? prevSelected
          : [...prevSelected, name]
        : prevSelected.filter((n) => n !== name);

      setPreferredDefault((prevPreferred) => {
        if (checked) {
          if (prevPreferred === null || prevPreferred === name) {
            return name;
          }
          if (!nextSelected.includes(prevPreferred)) {
            return name;
          }
          return prevPreferred;
        }
        if (prevPreferred === name) {
          return nextSelected[0] ?? null;
        }
        return prevPreferred;
      });

      return nextSelected;
    });
  };

  const openWizard = () => {
    setStatusMessage(null);
    setImportOutcome(null);
    setBrowsersReady(false);
    setRunningBrowsers([]);
    setWizardStep('select');
    setWizardOpen(true);
  };

  const closeWizard = () => {
    setWizardOpen(false);
    setWizardStep('select');
    setImportOutcome(null);
    setBrowsersReady(false);
    setRunningBrowsers([]);
  };

  const goToCloseStep = async () => {
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
    setStatusMessage(null);
    setWizardStep('close');
    await refreshReadiness();
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
      setBrowsersReady(report.ready);
      if (report.ready) {
        setStatusTone('ok');
        setStatusMessage(
          t(
            'settings.system.browserProfiles.quitSuccess',
            'Those browsers are closed. Continue to copy your logins.',
          ),
        );
        return;
      }
      setStatusTone('warn');
      setStatusMessage(
        t(
          'settings.system.browserProfiles.quitPartial',
          'Still open: {{browsers}}. Quit again, or close any remaining windows yourself.',
          { browsers: formatBrowserList(report.stillRunning) },
        ),
      );
    } catch (error) {
      logger.error('Failed to quit browsers for import', error);
      setStatusTone('error');
      setStatusMessage(
        t(
          'settings.system.browserProfiles.quitError',
          'Could not close the browsers automatically. Close them yourself, then check again.',
        ),
      );
    } finally {
      setQuitting(false);
    }
  };

  const goToCopyStep = async () => {
    const ready = await refreshReadiness();
    if (!ready) {
      setStatusTone('warn');
      setStatusMessage(
        t(
          'settings.system.browserProfiles.closeNeededBody',
          'Still open: {{browsers}}. Close them before copying.',
          { browsers: formatBrowserList(runningBrowsers) },
        ),
      );
      return;
    }
    setStatusMessage(null);
    setWizardStep('copy');
  };

  const runImport = async () => {
    setImporting(true);
    setStatusMessage(null);
    try {
      const names = selectedNames.length > 0 ? selectedNames : undefined;
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
      setBrowsersReady(stillOpen.length === 0);

      if (report.imported.length > 0 && report.skipped.length === 0) {
        setImportOutcome('success');
        setStatusTone('ok');
        setStatusMessage(
          t(
            'settings.system.browserProfiles.importSuccess',
            'Done — your browser logins are ready for LibrAgent. When an agent needs them, you will be asked to confirm first.',
          ),
        );
        setWizardStep('done');
        return;
      }

      if (report.imported.length > 0 && report.skipped.length > 0) {
        setImportOutcome('partial');
        setStatusTone('warn');
        setStatusMessage(
          t(
            'settings.system.browserProfiles.importPartial',
            'Some logins were saved, but others could not be copied because a browser was still open.',
          ),
        );
        setWizardStep('done');
        return;
      }

      if (stillOpen.length > 0 || report.skipped.length > 0) {
        setImportOutcome('blocked');
        setStatusTone('warn');
        setStatusMessage(
          t(
            'settings.system.browserProfiles.importBlocked',
            'Copy could not finish because a browser is still open. Go back one step, close it, then try again.',
          ),
        );
        setWizardStep('close');
        return;
      }

      if (report.warnings.length > 0) {
        setImportOutcome('none');
        setStatusTone('warn');
        setStatusMessage(
          t(
            'settings.system.browserProfiles.importNoneFound',
            'No supported browser profiles were found. Install Chrome, Edge, or Brave and sign in there first.',
          ),
        );
        setWizardStep('done');
        return;
      }

      setImportOutcome('none');
      setStatusTone('warn');
      setStatusMessage(
        t(
          'settings.system.browserProfiles.importNone',
          'Nothing was imported. Make sure a browser is installed and you have signed in at least once.',
        ),
      );
      setWizardStep('done');
    } catch (error) {
      logger.error('Failed to import browser profiles', error);
      const ready = await refreshReadiness();
      setImportOutcome(ready ? 'error' : 'blocked');
      setStatusTone('error');
      if (!ready) {
        setStatusMessage(
          t(
            'settings.system.browserProfiles.importBlocked',
            'Copy could not finish because a browser is still open. Go back one step, close it, then try again.',
          ),
        );
        setWizardStep('close');
      } else {
        setStatusMessage(
          t(
            'settings.system.browserProfiles.importError',
            'Copy failed. Close the browsers, wait a few seconds, then try again.',
          ),
        );
        setWizardStep('done');
      }
    } finally {
      setImporting(false);
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
    setRemoveDialogOpen(false);
    setProfilePendingRemoval(null);
    setRemovingName(name);
    setStatusMessage(null);
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
      setSignInSessionFor(name);
      setSignInHelpOpenFor(name);
      setStatusTone('ok');
      setStatusMessage(
        t(
          'settings.system.browserProfiles.signInOpened',
          'Chrome opened for this saved copy. Sign in there, then tap “I’m done signing in” below.',
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

  const handleCloseSignInWindow = async (name: string) => {
    setClosingSignInName(name);
    setStatusMessage(null);
    try {
      const report = await quitBrowserProfileSignInWindows(name, true);
      await refresh();
      if (report.closed) {
        setSignInSessionFor((current) => (current === name ? null : current));
        setStatusTone('ok');
        setStatusMessage(
          t(
            'settings.system.browserProfiles.closeSignInSuccess',
            'Done. Agents can use this saved login after you approve browser access.',
          ),
        );
      } else {
        setStatusTone('warn');
        setStatusMessage(
          t(
            'settings.system.browserProfiles.closeSignInPartial',
            'Still open. Close any remaining LibrAgent Chrome window for this login, then Refresh.',
          ),
        );
      }
    } catch (error) {
      logger.error('Failed to close browser profile sign-in window', error);
      setStatusTone('error');
      setStatusMessage(
        t(
          'settings.system.browserProfiles.closeSignInError',
          'Could not close the login window automatically. Close the LibrAgent Chrome window yourself, then Refresh.',
        ),
      );
    } finally {
      setClosingSignInName(null);
    }
  };

  const busy =
    importing ||
    removingName !== null ||
    checkingReady ||
    quitting ||
    settingDefault !== null ||
    signingInName !== null ||
    closingSignInName !== null;

  return (
    <div className="rounded-xl border border-border/70 p-4 max-w-lg space-y-4">
      <div className="space-y-1">
        <h4 className="text-sm font-medium text-foreground">
          {t('settings.system.browserProfiles.title', 'Saved browser logins')}
        </h4>
        <p className="text-xs text-muted-foreground leading-relaxed">
          {t(
            'settings.system.browserProfiles.description',
            'This is a short guided process — not one click. You choose a browser, close it so files can be read, then LibrAgent copies logins into a private folder. Your everyday browser stays unchanged.',
          )}
        </p>
      </div>

      {!wizardOpen ? (
        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            variant="default"
            disabled={busy || loading || discoverable.length === 0}
            onClick={openWizard}
          >
            {t(
              'settings.system.browserProfiles.startImportWizard',
              'Start import…',
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
      ) : null}

      {!wizardOpen && discoverable.length === 0 && !loading ? (
        <p className="text-xs text-muted-foreground">
          {t(
            'settings.system.browserProfiles.noneDiscoverable',
            'No supported browsers found on this computer yet.',
          )}
        </p>
      ) : null}

      {wizardOpen ? (
        <div className="rounded-lg border border-border/60 bg-muted/20 p-3 space-y-4">
          <div className="space-y-2">
            <div className="flex items-center justify-between gap-2">
              <p className="text-xs font-medium text-foreground">
                {t(
                  'settings.system.browserProfiles.wizardTitle',
                  'Import browser logins',
                )}
              </p>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="h-7 px-2 text-xs"
                disabled={busy && wizardStep !== 'done'}
                onClick={closeWizard}
              >
                {t('settings.system.browserProfiles.wizardCancel', 'Cancel')}
              </Button>
            </div>
            <WizardStepList active={wizardStep} labels={wizardStepLabels} />
          </div>

          {wizardStep === 'select' ? (
            <div className="space-y-3">
              <div className="space-y-1">
                <p className="text-sm font-medium text-foreground">
                  {t(
                    'settings.system.browserProfiles.wizardSelectTitle',
                    'Step 1 — Choose what to copy',
                  )}
                </p>
                <p className="text-xs text-muted-foreground">
                  {t(
                    'settings.system.browserProfiles.selectHint',
                    'Only selected browsers are closed in the next step. Importing Edge will not ask you to quit Chrome.',
                  )}
                </p>
              </div>
              {discoverable.length > 0 ? (
                <ul className="space-y-2">
                  {discoverable.map((profile) => {
                    const checked = selectedNames.includes(profile.name);
                    return (
                      <li
                        key={profile.name}
                        className="flex items-start gap-3 rounded-lg bg-background/70 px-3 py-2"
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
                                onChange={() =>
                                  setPreferredDefault(profile.name)
                                }
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
              ) : (
                <p className="text-xs text-muted-foreground">
                  {t(
                    'settings.system.browserProfiles.noneDiscoverable',
                    'No supported browsers found on this computer yet.',
                  )}
                </p>
              )}
              <div className="flex flex-wrap gap-2">
                <Button
                  type="button"
                  disabled={busy || selectedNames.length === 0}
                  onClick={() => void goToCloseStep()}
                >
                  {t(
                    'settings.system.browserProfiles.wizardNextClose',
                    'Next: close browsers',
                  )}
                </Button>
              </div>
            </div>
          ) : null}

          {wizardStep === 'close' ? (
            <div className="space-y-3">
              <div className="space-y-1">
                <p className="text-sm font-medium text-foreground">
                  {t(
                    'settings.system.browserProfiles.wizardCloseTitle',
                    'Step 2 — Close those browsers',
                  )}
                </p>
                <p className="text-xs text-muted-foreground leading-relaxed">
                  {t(
                    'settings.system.browserProfiles.wizardCloseBody',
                    'Browsers lock their login files while open. They must be fully quit before LibrAgent can copy anything. Unsaved tabs may be lost.',
                  )}
                </p>
              </div>

              {checkingReady ? (
                <p className="text-xs text-muted-foreground">
                  {t(
                    'settings.system.browserProfiles.checking',
                    'Checking…',
                  )}
                </p>
              ) : browsersReady ? (
                <p className="text-xs text-emerald-700 dark:text-emerald-400">
                  {t(
                    'settings.system.browserProfiles.wizardCloseReady',
                    'Ready — selected browsers are closed.',
                  )}
                </p>
              ) : (
                <div className="rounded-md border border-amber-500/40 bg-amber-500/10 p-3 space-y-2">
                  <p className="text-xs font-medium text-foreground">
                    {t(
                      'settings.system.browserProfiles.closeNeededTitle',
                      'Still open',
                    )}
                  </p>
                  <p className="text-xs text-muted-foreground">
                    {t(
                      'settings.system.browserProfiles.closeNeededBody',
                      'Still open: {{browsers}}. Close them before copying.',
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
                </div>
              )}

              <div className="flex flex-wrap gap-2">
                <Button
                  type="button"
                  variant="outline"
                  disabled={busy}
                  onClick={() => setWizardStep('select')}
                >
                  {t('settings.system.browserProfiles.wizardBack', 'Back')}
                </Button>
                {!browsersReady ? (
                  <Button
                    type="button"
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
                ) : null}
                <Button
                  type="button"
                  variant={browsersReady ? 'default' : 'outline'}
                  disabled={busy}
                  onClick={() => void goToCopyStep()}
                >
                  {checkingReady
                    ? t(
                        'settings.system.browserProfiles.checking',
                        'Checking…',
                      )
                    : browsersReady
                      ? t(
                          'settings.system.browserProfiles.wizardNextCopy',
                          'Next: copy logins',
                        )
                      : t(
                          'settings.system.browserProfiles.wizardRecheck',
                          'I’ve closed them — check again',
                        )}
                </Button>
              </div>
            </div>
          ) : null}

          {wizardStep === 'copy' ? (
            <div className="space-y-3">
              <div className="space-y-1">
                <p className="text-sm font-medium text-foreground">
                  {t(
                    'settings.system.browserProfiles.wizardCopyTitle',
                    'Step 3 — Copy into LibrAgent',
                  )}
                </p>
                <p className="text-xs text-muted-foreground leading-relaxed">
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
                </p>
              </div>
              <div className="flex flex-wrap gap-2">
                <Button
                  type="button"
                  variant="outline"
                  disabled={busy}
                  onClick={() => setWizardStep('close')}
                >
                  {t('settings.system.browserProfiles.wizardBack', 'Back')}
                </Button>
                <Button
                  type="button"
                  disabled={busy}
                  onClick={() => void runImport()}
                >
                  {importing
                    ? t(
                        'settings.system.browserProfiles.importing',
                        'Copying…',
                      )
                    : t(
                        'settings.system.browserProfiles.importConfirmAction',
                        'Copy logins now',
                      )}
                </Button>
              </div>
            </div>
          ) : null}

          {wizardStep === 'done' ? (
            <div className="space-y-3">
              <div className="space-y-1">
                <p className="text-sm font-medium text-foreground">
                  {t(
                    'settings.system.browserProfiles.wizardDoneTitle',
                    'Step 4 — Finished',
                  )}
                </p>
                <p
                  className={
                    importOutcome === 'success'
                      ? 'text-xs text-emerald-700 dark:text-emerald-400 leading-relaxed'
                      : importOutcome === 'error' ||
                          importOutcome === 'blocked'
                        ? 'text-xs text-destructive leading-relaxed'
                        : 'text-xs text-amber-700 dark:text-amber-400 leading-relaxed'
                  }
                >
                  {statusMessage ??
                    t(
                      'settings.system.browserProfiles.importSuccess',
                      'Done — your browser logins are ready for LibrAgent.',
                    )}
                </p>
              </div>
              <div className="flex flex-wrap gap-2">
                {importOutcome === 'blocked' || importOutcome === 'partial' ? (
                  <Button
                    type="button"
                    variant="outline"
                    disabled={busy}
                    onClick={() => {
                      setImportOutcome(null);
                      setWizardStep('close');
                      void refreshReadiness();
                    }}
                  >
                    {t(
                      'settings.system.browserProfiles.wizardRetryClose',
                      'Back to close browsers',
                    )}
                  </Button>
                ) : null}
                <Button type="button" onClick={closeWizard}>
                  {t(
                    'settings.system.browserProfiles.wizardFinish',
                    'Close',
                  )}
                </Button>
              </div>
            </div>
          ) : null}

          {wizardStep !== 'done' && statusMessage ? (
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
        </div>
      ) : null}

      {!wizardOpen && statusMessage ? (
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

      {profiles.length > 0 ? (
        <div className="space-y-2">
          {!wizardOpen ? (
            <p className="text-xs font-medium text-foreground">
              {t(
                'settings.system.browserProfiles.savedListTitle',
                'Already saved',
              )}
            </p>
          ) : null}
          <ul className="space-y-2 text-sm">
            {profiles.map((profile) => (
              <li
                key={profile.name}
                className="rounded-lg bg-muted/40 px-3 py-2 space-y-2"
              >
                <div className="flex items-center justify-between gap-3">
                  <div className="min-w-0">
                    <p className="font-medium truncate">{profile.label}</p>
                    <p className="text-xs text-muted-foreground truncate">
                      {profile.sourceBrowser}
                      {profile.isDefault
                        ? ` · ${t('settings.system.browserProfiles.defaultBadge', 'primary')}`
                        : ''}
                    </p>
                    <p className="text-xs text-muted-foreground truncate">
                      {t('settings.system.browserProfiles.importedAt', 'Saved')}
                      : {formatImportedAt(profile.importedAt)}
                    </p>
                  </div>
                  <div className="flex shrink-0 items-center gap-1">
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
                        setRemoveDialogOpen(true);
                      }}
                    >
                      {removingName === profile.name
                        ? t(
                            'settings.system.browserProfiles.removing',
                            'Removing…',
                          )
                        : t(
                            'settings.system.browserProfiles.remove',
                            'Remove',
                          )}
                    </Button>
                  </div>
                </div>
                {profile.importKind !== 'firefox_cookies' ? (
                  <div className="border-t border-border/50 pt-2">
                    <button
                      type="button"
                      className="text-xs text-muted-foreground underline-offset-2 hover:underline"
                      disabled={busy}
                      onClick={() =>
                        setSignInHelpOpenFor((current) =>
                          current === profile.name ? null : profile.name,
                        )
                      }
                    >
                      {t(
                        'settings.system.browserProfiles.signInHelpToggle',
                        'Google still asks you to sign in?',
                      )}
                    </button>
                    {signInHelpOpenFor === profile.name ? (
                      <div className="mt-2 space-y-2">
                        <p className="text-xs text-muted-foreground leading-relaxed">
                          {t(
                            'settings.system.browserProfiles.signInHelpBody',
                            'Rare extra step: open a LibrAgent-only Chrome window, sign in once, then confirm below. Your everyday Chrome is not used.',
                          )}
                        </p>
                        {signInSessionFor === profile.name ? (
                          <Button
                            type="button"
                            size="sm"
                            disabled={busy}
                            onClick={() =>
                              void handleCloseSignInWindow(profile.name)
                            }
                          >
                            {closingSignInName === profile.name
                              ? t(
                                  'settings.system.browserProfiles.closingSignIn',
                                  'Closing…',
                                )
                              : t(
                                  'settings.system.browserProfiles.signInDone',
                                  'I’m done signing in',
                                )}
                          </Button>
                        ) : (
                          <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            disabled={busy}
                            onClick={() =>
                              void handleOpenForSignIn(profile.name)
                            }
                          >
                            {signingInName === profile.name
                              ? t(
                                  'settings.system.browserProfiles.signingIn',
                                  'Opening…',
                                )
                              : t(
                                  'settings.system.browserProfiles.openToSignIn',
                                  'Open Chrome to sign in',
                                )}
                          </Button>
                        )}
                      </div>
                    ) : null}
                  </div>
                ) : null}
              </li>
            ))}
          </ul>
        </div>
      ) : (
        !loading &&
        !wizardOpen && (
          <p className="text-xs text-muted-foreground">
            {t(
              'settings.system.browserProfiles.empty',
              'Nothing saved yet. Start the import when you are ready.',
            )}
          </p>
        )
      )}

      <AlertDialog
        open={removeDialogOpen}
        onOpenChange={(open) => {
          setRemoveDialogOpen(open);
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
