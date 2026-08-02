import { toast } from 'sonner';
import { openPathWithDefaultApp } from '@/lib/backend';
import { getLogger } from '@/lib/logger';

const logger = getLogger('notifyFileDownload');

export const DOWNLOAD_CANCELLED = 'Download cancelled by user';

function fileNameFromPath(filePath: string): string {
  const parts = filePath.split(/[/\\]/);
  return parts[parts.length - 1] || filePath;
}

/**
 * Shows a success toast for a saved file, with an action to open it
 * in the system default application.
 */
export function notifyFileDownloadSuccess(options: {
  title: string;
  filePath: string;
  openLabel: string;
  openErrorLabel: string;
}): void {
  const { title, filePath, openLabel, openErrorLabel } = options;

  toast.success(title, {
    description: fileNameFromPath(filePath),
    action: {
      label: openLabel,
      onClick: () => {
        void openPathWithDefaultApp(filePath).catch((error: unknown) => {
          logger.error('Failed to open downloaded file', { filePath, error });
          toast.error(openErrorLabel);
        });
      },
    },
  });
}
