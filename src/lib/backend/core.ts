import { invoke } from '@tauri-apps/api/core';
import { getLogger } from '@/lib/logger';
import { recordStartupIpcCall } from '@/lib/performance/startup-metrics';

const logger = getLogger('RustBackendClient');

interface SafeInvokeOptions {
  shouldSuppressErrorLogging?: (error: unknown) => boolean;
}

/**
 * A wrapper around Tauri's `invoke` function that provides centralized
 * logging and error handling for all backend calls.
 *
 * @template T The expected return type of the invoked command.
 * @param cmd The name of the command to invoke on the backend.
 * @param args Optional arguments for the command.
 * @returns A promise that resolves with the result of the command.
 * @throws Rethrows the error from the backend if the invocation fails.
 * @internal
 */
export async function safeInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
  options?: SafeInvokeOptions,
): Promise<T> {
  const startedAt =
    typeof performance !== 'undefined' ? performance.now() : Date.now();

  try {
    logger.debug('invoke', { cmd, args });
    const result = await invoke<T>(cmd, args ?? {});
    const durationMs =
      (typeof performance !== 'undefined' ? performance.now() : Date.now()) -
      startedAt;
    recordStartupIpcCall(cmd, durationMs, true);
    return result;
  } catch (err) {
    const durationMs =
      (typeof performance !== 'undefined' ? performance.now() : Date.now()) -
      startedAt;
    recordStartupIpcCall(cmd, durationMs, false);
    if (options?.shouldSuppressErrorLogging?.(err) === true) {
      logger.info('invoke ended without error-level logging', { cmd, err });
    } else {
      logger.error('invoke failed', { cmd, err });
    }
    throw err;
  }
}
