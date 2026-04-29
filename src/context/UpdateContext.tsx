import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import {
  check,
  type Update,
  type DownloadEvent,
} from '@tauri-apps/plugin-updater';
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';
import { safeInvoke } from '@/lib/backend/core';
import {
  getUpdateInstallCapability,
  openExternalUrl,
  type UpdateInstallCapability,
} from '@/lib/backend/utils';

const logger = getLogger('UpdateContext');

// Delay before the first auto-check so the app can finish initializing.
const AUTO_CHECK_DELAY_MS = 5_000;
const RELEASES_URL = 'https://github.com/fritzprix/libr-agent/releases';

export type UpdateStatus =
  | 'idle'
  | 'checking'
  | 'available'
  | 'up-to-date'
  | 'downloading'
  | 'installing'
  | 'error';

interface UpdateState {
  status: UpdateStatus;
  /** The new version string if an update is available. */
  availableVersion: string | null;
  /** Download progress 0-100, only meaningful when status === 'downloading'. */
  downloadProgress: number;
  /** Error message when status === 'error'. */
  error: string | null;
  /** Whether this installation can install updates in-app. */
  canInstallUpdate: boolean;
  /** Guidance for installations that cannot self-update in-app. */
  installHint: string | null;
}

interface UpdateContextValue extends UpdateState {
  /** Manually trigger an update check. */
  checkForUpdate: () => Promise<void>;
  /** Start downloading + installing the pending update (if any). */
  installUpdate: () => Promise<void>;
}

const UpdateContext = createContext<UpdateContextValue | null>(null);

export function useUpdateContext(): UpdateContextValue {
  const ctx = useContext(UpdateContext);
  if (!ctx) {
    throw new Error('useUpdateContext must be used inside <UpdateProvider>');
  }
  return ctx;
}

interface UpdateProviderProps {
  children: ReactNode;
}

export function UpdateProvider({ children }: UpdateProviderProps) {
  const [status, setStatus] = useState<UpdateStatus>('idle');
  const [availableVersion, setAvailableVersion] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [canInstallUpdate, setCanInstallUpdate] = useState(true);
  const [installHint, setInstallHint] = useState<string | null>(null);

  // Keep a ref to the pending Update object so installUpdate can access it
  const pendingUpdate = useRef<Update | null>(null);
  const autoChecked = useRef(false);
  const installCapabilityRef = useRef<UpdateInstallCapability | null>(null);
  const capabilityRequestRef = useRef<Promise<void> | null>(null);

  const openReleaseNotes = useCallback(async (version: string) => {
    const tagUrl = `${RELEASES_URL}/tag/v${encodeURIComponent(version)}`;
    await openExternalUrl(tagUrl);
  }, []);

  const ensureInstallCapabilityResolved = useCallback(async () => {
    if (capabilityRequestRef.current) {
      await capabilityRequestRef.current;
      return installCapabilityRef.current;
    }

    const request = (async () => {
      try {
        const capability = await getUpdateInstallCapability();
        installCapabilityRef.current = capability;
        setCanInstallUpdate(capability.supported);
        setInstallHint(capability.reason);
      } catch (err: unknown) {
        logger.warn('Failed to resolve update install capability:', err);
      } finally {
        capabilityRequestRef.current = null;
      }
    })();

    capabilityRequestRef.current = request;
    await request;
    return installCapabilityRef.current;
  }, []);

  const doInstall = useCallback(
    async (update: Update) => {
      const capability = await ensureInstallCapabilityResolved();
      const installSupported = capability?.supported ?? canInstallUpdate;
      const currentInstallHint = capability?.reason ?? installHint;

      if (!installSupported) {
        const message =
          currentInstallHint ??
          'This installation cannot apply updates in-app. Please install the latest release manually.';
        logger.warn('Blocked in-app update install:', message);
        setStatus('error');
        setError(message);
        toast.error(message, { duration: 6000 });
        return;
      }

      setStatus('downloading');
      setDownloadProgress(0);

      const toastId = toast.loading(
        `Downloading LibrAgent ${update.version}…`,
        {
          duration: Infinity,
        },
      );

      try {
        let downloaded = 0;
        let total: number | undefined;

        await update.downloadAndInstall((event: DownloadEvent) => {
          switch (event.event) {
            case 'Started':
              total = event.data.contentLength ?? undefined;
              logger.info(
                `Download started, size: ${total?.toString() ?? 'unknown'} bytes`,
              );
              break;
            case 'Progress': {
              downloaded += event.data.chunkLength;
              if (total) {
                const pct = Math.round((downloaded / total) * 100);
                setDownloadProgress(pct);
                toast.loading(`Downloading… ${pct.toString()}%`, {
                  id: toastId,
                  duration: Infinity,
                });
              }
              break;
            }
            case 'Finished':
              logger.info('Download finished, installing...');
              setStatus('installing');
              break;
          }
        });

        toast.success('Update installed! Restarting…', {
          id: toastId,
          duration: 2000,
        });
        await safeInvoke<void>('restart_app');
      } catch (err: unknown) {
        const rawMessage = err instanceof Error ? err.message : String(err);
        const msg =
          rawMessage.includes('os error 13') ||
          rawMessage.includes('Permission denied')
            ? (currentInstallHint ??
              'Update install failed because this Linux installation is not writable. If you installed LibrAgent via .deb/.rpm, update it with your package manager. For in-app updates, run the AppImage from a writable folder in your home directory.')
            : rawMessage;
        logger.error('Update installation failed:', err);
        setStatus('error');
        setError(msg);
        toast.error(msg, {
          id: toastId,
          duration: 7000,
        });
      }
    },
    [canInstallUpdate, ensureInstallCapabilityResolved, installHint],
  );

  const checkForUpdate = useCallback(async () => {
    if (
      status === 'checking' ||
      status === 'downloading' ||
      status === 'installing'
    )
      return;

    setStatus('checking');
    setError(null);
    try {
      const capability = await ensureInstallCapabilityResolved();
      const installSupported = capability?.supported ?? canInstallUpdate;
      logger.debug('Checking for updates...');
      const update = await check();

      if (!update) {
        logger.debug('Already up to date.');
        setStatus('up-to-date');
        setAvailableVersion(null);
        pendingUpdate.current = null;
        return;
      }

      logger.info(`Update available: current → ${update.version}`);
      pendingUpdate.current = update;
      setAvailableVersion(update.version);
      setStatus('available');

      const description = installSupported
        ? 'A new version is ready to install.'
        : 'A new version is available. Install it from the release page.';

      toast.info(`LibrAgent ${update.version} is available`, {
        description,
        duration: Infinity,
        action: installSupported
          ? {
              label: 'Install',
              onClick: () => {
                void doInstall(update);
              },
            }
          : {
              label: 'View changelog',
              onClick: () => {
                void openReleaseNotes(update.version);
              },
            },
        cancel: {
          label: installSupported ? 'View changelog' : 'Later',
          onClick: () => {
            if (installSupported) {
              void openReleaseNotes(update.version);
              return;
            }

            logger.info('User deferred update via toast.');
          },
        },
      });
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      logger.warn('Update check failed (non-fatal):', err);
      setStatus('error');
      setError(msg);
    }
  }, [
    canInstallUpdate,
    doInstall,
    ensureInstallCapabilityResolved,
    openReleaseNotes,
    status,
  ]);

  const installUpdate = useCallback(async () => {
    const update = pendingUpdate.current;
    if (!update) {
      logger.warn('installUpdate called but no pending update is available.');
      return;
    }
    await doInstall(update);
  }, [doInstall]);

  // Auto-check once on mount, after a short delay.
  useEffect(() => {
    // Disable auto-check in development to avoid unnecessary error logs
    // when the update endpoint is not yet available or reachable.
    if (import.meta.env.DEV) {
      logger.debug('Skipping auto update check in development mode.');
      return;
    }

    if (autoChecked.current) return;
    autoChecked.current = true;

    const timer = setTimeout(() => {
      void checkForUpdate();
    }, AUTO_CHECK_DELAY_MS);

    return () => clearTimeout(timer);
  }, [checkForUpdate]);

  const value = useMemo<UpdateContextValue>(
    () => ({
      status,
      availableVersion,
      downloadProgress,
      error,
      canInstallUpdate,
      installHint,
      checkForUpdate,
      installUpdate,
    }),
    [
      status,
      availableVersion,
      downloadProgress,
      error,
      canInstallUpdate,
      installHint,
      checkForUpdate,
      installUpdate,
    ],
  );

  return (
    <UpdateContext.Provider value={value}>{children}</UpdateContext.Provider>
  );
}
