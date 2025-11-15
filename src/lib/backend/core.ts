import { invoke } from '@tauri-apps/api/core';
import { getLogger } from '@/lib/logger';

const logger = getLogger('RustBackendClient');

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
): Promise<T> {
  try {
    logger.debug('invoke', { cmd, args });
    return await invoke<T>(cmd, args ?? {});
  } catch (err) {
    logger.error('invoke failed', { cmd, err });
    throw err;
  }
}
