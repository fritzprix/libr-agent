import { invoke } from '@tauri-apps/api/core';
import { getLogger } from '@/lib/logger';
import { useCallback } from 'react';

const logger = getLogger('useBrowserInvoker');

export interface BrowserInvoker {
  executeScript: (sessionId: string, script: string) => Promise<string>;
}

/**
 * Hook that provides browser script execution with direct result delivery.
 * Uses oneshot channel pattern in Rust backend - no polling required.
 */
export function useBrowserInvoker(): BrowserInvoker {
  const executeScript = useCallback(
    async (sessionId: string, script: string): Promise<string> => {
      try {
        logger.debug('Executing script in session', {
          sessionId,
          script: script.substring(0, 100) + '...',
        });

        // Direct result delivery - no polling!
        const result = await invoke<string>('execute_script', {
          sessionId,
          script,
        });

        logger.debug('Script execution completed', {
          sessionId,
          resultLength: result.length,
        });

        return result;
      } catch (error) {
        logger.error('Failed to execute script', { sessionId, error });
        throw error;
      }
    },
    [],
  );

  return { executeScript };
}
