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
import { check, type Update } from '@tauri-apps/plugin-updater';
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';
import { safeInvoke } from '@/lib/backend/core';

const logger = getLogger('UpdateContext');

// Delay before the first auto-check so the app can finish initializing.
const AUTO_CHECK_DELAY_MS = 5_000;

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

  // Keep a ref to the pending Update object so installUpdate can access it
  const pendingUpdate = useRef<Update | null>(null);
  const autoChecked = useRef(false);

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

      // Show a non-blocking toast so the user can act from anywhere in the app
      const releaseNotes = update.body
        ? `${update.body.slice(0, 300)}${update.body.length > 300 ? '…' : ''}`
        : undefined;

      toast.info(`LibrAgent ${update.version} is available`, {
        description: releaseNotes ?? 'A new version is ready to install.',
        duration: Infinity,
        action: {
          label: 'Install',
          onClick: () => {
            void doInstall(update);
          },
        },
        cancel: {
          label: 'Later',
          onClick: () => logger.info('User deferred update via toast.'),
        },
      });
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      logger.warn('Update check failed (non-fatal):', err);
      setStatus('error');
      setError(msg);
    }
  }, [status]);

  const doInstall = useCallback(async (update: Update) => {
    setStatus('downloading');
    setDownloadProgress(0);

    const toastId = toast.loading(`Downloading LibrAgent ${update.version}…`, {
      duration: Infinity,
    });

    try {
      let downloaded = 0;
      let total: number | undefined;

      await update.downloadAndInstall((event) => {
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
      await safeInvoke('restart_app');
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      logger.error('Update installation failed:', err);
      setStatus('error');
      setError(msg);
      toast.error('Update failed. Please restart and try again.', {
        id: toastId,
        duration: 5000,
      });
    }
  }, []);

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
    if (autoChecked.current) return;
    autoChecked.current = true;

    const timer = setTimeout(() => {
      void checkForUpdate();
    }, AUTO_CHECK_DELAY_MS);

    return () => clearTimeout(timer);
  }, []);

  const value = useMemo<UpdateContextValue>(
    () => ({
      status,
      availableVersion,
      downloadProgress,
      error,
      checkForUpdate,
      installUpdate,
    }),
    [
      status,
      availableVersion,
      downloadProgress,
      error,
      checkForUpdate,
      installUpdate,
    ],
  );

  return (
    <UpdateContext.Provider value={value}>{children}</UpdateContext.Provider>
  );
}
