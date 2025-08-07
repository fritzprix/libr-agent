import { useState } from 'react';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useClipboard');

interface UseClipboardOptions {
  successDuration?: number;
}

export const useClipboard = (options: UseClipboardOptions = {}) => {
  const { successDuration = 2000 } = options;
  const [copied, setCopied] = useState(false);

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), successDuration);
      logger.debug('Content copied to clipboard', { textLength: text.length });
    } catch (err) {
      logger.error('Failed to copy to clipboard', err);
      throw err;
    }
  };

  return { copied, copyToClipboard };
};
