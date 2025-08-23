import { invoke } from '@tauri-apps/api/core';
import { getLogger } from '@/lib/logger';
import { useCallback } from 'react';

const logger = getLogger('useBrowserInvoker');

const POLLING_INTERVAL = 100; // ms
const TIMEOUT = 10000; // ms

export interface BrowserInvoker {
  executeScript: (sessionId: string, script: string) => Promise<string>;
}

/**
 * Hook that provides non-blocking browser script execution using polling mechanism.
 * This resolves the deadlock issue where the Rust backend blocks waiting for results
 * while the WebView tries to send those results back.
 */
export function useBrowserInvoker(): BrowserInvoker {
  const executeScript = useCallback(async (
    sessionId: string,
    script: string,
  ): Promise<string> => {
    try {
      logger.debug('Executing script in session', {
        sessionId,
        script: script.substring(0, 100) + '...',
      });

      // Call the non-blocking execute_script command to get request_id
      const requestId = await invoke<string>('execute_script', {
        sessionId,
        script,
      });

      logger.debug('Received request ID, starting polling', {
        sessionId,
        requestId,
      });

      // Poll for the result using the request_id
      return new Promise((resolve, reject) => {
        const startTime = Date.now();

        const poll = setInterval(async () => {
          try {
            // Check for timeout
            if (Date.now() - startTime > TIMEOUT) {
              clearInterval(poll);
              const errorMsg = `Timeout waiting for script result for request: ${requestId}`;
              logger.error(errorMsg, { sessionId, requestId });
              reject(new Error(errorMsg));
              return;
            }

            // Poll for result
            const result = await invoke<string | null>('poll_script_result', {
              requestId,
            });

            if (result !== null) {
              clearInterval(poll);
              logger.debug('Script execution completed', {
                sessionId,
                requestId,
                resultLength: result.length,
              });
              resolve(result);
            }
          } catch (error) {
            clearInterval(poll);
            logger.error('Error during polling', {
              sessionId,
              requestId,
              error,
            });
            reject(error);
          }
        }, POLLING_INTERVAL);
      });
    } catch (error) {
      logger.error('Failed to execute script', { sessionId, error });
      throw error;
    }
  }, []); // Empty dependency array since this function doesn't depend on any props/state

  return { executeScript };
}
