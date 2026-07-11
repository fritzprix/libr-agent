import { safeInvoke } from './core';

const DEFAULT_POLL_INTERVAL_MS = 5000;
const DEFAULT_MAX_ATTEMPTS = 6;

export interface WaitForDockerReadyOptions {
  maxAttempts?: number;
  intervalMs?: number;
  sleep?: (ms: number) => Promise<void>;
  signal?: AbortSignal;
}

/**
 * Verifies that the Docker CLI is available and the daemon is reachable.
 */
export async function checkDockerHealth(): Promise<void> {
  await safeInvoke<void>('check_docker_health');
}

/**
 * Polls Docker health until the daemon is ready or attempts are exhausted.
 */
export async function waitForDockerReady(
  options: WaitForDockerReadyOptions = {},
): Promise<boolean> {
  const {
    maxAttempts = DEFAULT_MAX_ATTEMPTS,
    intervalMs = DEFAULT_POLL_INTERVAL_MS,
    sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
    signal,
  } = options;

  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    if (signal?.aborted) {
      return false;
    }

    try {
      await checkDockerHealth();
      return true;
    } catch {
      if (attempt === maxAttempts) {
        return false;
      }
      await sleep(intervalMs);
    }
  }

  return false;
}
