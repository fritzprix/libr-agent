import { useEffect, useRef } from 'react';
import { check } from '@tauri-apps/plugin-updater';
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';
import { safeInvoke } from '@/lib/backend/core';

const logger = getLogger('useAutoUpdater');

// Delay before the first update check so the app can finish initializing.
const CHECK_DELAY_MS = 5_000;

/**
 * Checks for a new LibrAgent release in the background, once per session.
 * If an update is available, shows a non-blocking toast with an "Install" button.
 * The update is downloaded and installed on confirmation; the app restarts automatically.
 *
 * Must be called in a component that is mounted for the lifetime of the app (e.g. App.tsx).
 */
export function useAutoUpdater(): void {
  const checked = useRef(false);

  useEffect(() => {
    if (checked.current) return;
    checked.current = true;

    const timer = setTimeout(async () => {
      try {
        logger.debug('Checking for updates...');
        const update = await check();

        if (!update) {
          logger.debug('No update available.');
          return;
        }

        logger.info(
          `Update available: ${update.currentVersion} → ${update.version}`,
        );

        const releaseNotes = update.body
          ? `\n\n${update.body.slice(0, 300)}${update.body.length > 300 ? '…' : ''}`
          : '';

        toast.info(`LibrAgent ${update.version} is available${releaseNotes}`, {
          description: 'Click Install to update now.',
          duration: Infinity,
          action: {
            label: 'Install',
            onClick: () => {
              void installUpdate(update);
            },
          },
          cancel: {
            label: 'Later',
            onClick: () => {
              logger.info('User deferred update.');
            },
          },
        });
      } catch (error: unknown) {
        // Silently ignore update check failures — network errors, etc.
        logger.warn('Update check failed (non-fatal):', error);
      }
    }, CHECK_DELAY_MS);

    return () => clearTimeout(timer);
  }, []);
}

async function installUpdate(
  update: Awaited<ReturnType<typeof check>>,
): Promise<void> {
  if (!update) return;

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
            `Download started, total size: ${total?.toString() ?? 'unknown'} bytes`,
          );
          break;
        case 'Progress':
          downloaded += event.data.chunkLength;
          if (total) {
            const pct = Math.round((downloaded / total) * 100);
            toast.loading(`Downloading… ${pct}%`, {
              id: toastId,
              duration: Infinity,
            });
          }
          break;
        case 'Finished':
          logger.info('Download finished, ready to install.');
          break;
      }
    });

    toast.success('Update installed! Restarting…', {
      id: toastId,
      duration: 2000,
    });

    // Restart the app via the existing restart_app Tauri command.
    await safeInvoke('restart_app');
  } catch (error: unknown) {
    logger.error('Update installation failed:', error);
    toast.error('Update failed. Please restart and try again.', {
      id: toastId,
      duration: 5000,
    });
  }
}
