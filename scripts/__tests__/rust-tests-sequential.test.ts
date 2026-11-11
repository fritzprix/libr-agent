import { createRequire } from 'node:module';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const require = createRequire(import.meta.url);
const {
  isTransientWindowsExecFailure,
  maxAttemptsFor,
  runCargoTestWithRetries,
  MAX_ATTEMPTS_WINDOWS,
  RETRY_BASE_DELAY_MS,
} = require('../run-rust-tests-sequential.cjs') as {
  isTransientWindowsExecFailure: (output: string) => boolean;
  maxAttemptsFor: (platform: NodeJS.Platform) => number;
  runCargoTestWithRetries: (
    cargoArgs: string[],
    targetLabel: string,
    deps?: {
      runCargoTest?: (cargoArgs: string[]) => { status: number; output: string };
      sleepMs?: (ms: number) => void;
      maxAttempts?: number;
    },
  ) => number;
  MAX_ATTEMPTS_WINDOWS: number;
  RETRY_BASE_DELAY_MS: number;
};

/** Output shape Windows Defender produces when it locks a freshly linked test exe. */
const TRANSIENT_OUTPUT =
  'error: failed to execute process: Cannot execute ...: os error 5';
const FATAL_OUTPUT = 'test result: FAILED. assertion failed: left == right';

describe('rust-tests-sequential retry policy', () => {
  let consoleError: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    consoleError.mockRestore();
  });

  describe('isTransientWindowsExecFailure', () => {
    it('classifies exec-lock failures as transient', () => {
      expect(isTransientWindowsExecFailure(TRANSIENT_OUTPUT)).toBe(true);
      expect(isTransientWindowsExecFailure('never executed')).toBe(true);
      expect(isTransientWindowsExecFailure('Access is denied.')).toBe(true);
      expect(isTransientWindowsExecFailure('액세스가 거부되었습니다')).toBe(true);
    });

    it('does not classify real test failures as transient', () => {
      expect(isTransientWindowsExecFailure(FATAL_OUTPUT)).toBe(false);
      expect(isTransientWindowsExecFailure('')).toBe(false);
    });
  });

  describe('maxAttemptsFor', () => {
    it('allows 5 attempts on Windows only', () => {
      expect(maxAttemptsFor('win32')).toBe(MAX_ATTEMPTS_WINDOWS);
      expect(MAX_ATTEMPTS_WINDOWS).toBe(5);
      expect(maxAttemptsFor('linux')).toBe(1);
      expect(maxAttemptsFor('darwin')).toBe(1);
    });
  });

  describe('runCargoTestWithRetries', () => {
    it('retries a persistent transient failure up to the attempt cap', () => {
      const sleeps: number[] = [];
      let calls = 0;
      const status = runCargoTestWithRetries(['--test', 'x'], 'x', {
        runCargoTest: () => {
          calls += 1;
          return { status: 1, output: TRANSIENT_OUTPUT };
        },
        sleepMs: (ms) => sleeps.push(ms),
        maxAttempts: MAX_ATTEMPTS_WINDOWS,
      });
      expect(calls).toBe(MAX_ATTEMPTS_WINDOWS);
      expect(status).toBe(1);
      // Linear backoff: attempt N waits N * 2s; no sleep after the last attempt.
      expect(sleeps).toEqual([
        RETRY_BASE_DELAY_MS,
        RETRY_BASE_DELAY_MS * 2,
        RETRY_BASE_DELAY_MS * 3,
        RETRY_BASE_DELAY_MS * 4,
      ]);
    });

    it('returns 0 as soon as a retry succeeds', () => {
      const sleeps: number[] = [];
      let calls = 0;
      const status = runCargoTestWithRetries(['--lib'], 'lib', {
        runCargoTest: () => {
          calls += 1;
          return calls <= 2
            ? { status: 1, output: TRANSIENT_OUTPUT }
            : { status: 0, output: '' };
        },
        sleepMs: (ms) => sleeps.push(ms),
        maxAttempts: MAX_ATTEMPTS_WINDOWS,
      });
      expect(status).toBe(0);
      expect(calls).toBe(3);
      expect(sleeps).toEqual([RETRY_BASE_DELAY_MS, RETRY_BASE_DELAY_MS * 2]);
    });

    it('returns early without retrying when the failure is not transient', () => {
      const sleeps: number[] = [];
      let calls = 0;
      const status = runCargoTestWithRetries(['--test', 'y'], 'y', {
        runCargoTest: () => {
          calls += 1;
          return { status: 101, output: FATAL_OUTPUT };
        },
        sleepMs: (ms) => sleeps.push(ms),
        maxAttempts: MAX_ATTEMPTS_WINDOWS,
      });
      expect(status).toBe(101);
      expect(calls).toBe(1);
      expect(sleeps).toEqual([]);
    });

    it('never retries on platforms where the attempt cap is 1', () => {
      const sleeps: number[] = [];
      let calls = 0;
      const status = runCargoTestWithRetries(['--lib'], 'lib', {
        runCargoTest: () => {
          calls += 1;
          return { status: 1, output: TRANSIENT_OUTPUT };
        },
        sleepMs: (ms) => sleeps.push(ms),
        maxAttempts: maxAttemptsFor('linux'),
      });
      expect(status).toBe(1);
      expect(calls).toBe(1);
      expect(sleeps).toEqual([]);
    });
  });
});
