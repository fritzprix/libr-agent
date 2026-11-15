import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const require = createRequire(import.meta.url);
const RUNNER_PATH = require.resolve('../run-rust-tests-sequential.cjs');
const {
  isTransientWindowsExecFailure,
  maxAttemptsFor,
  runCargoTestWithRetries,
  writeCapturedOutput,
  MAX_ATTEMPTS_WINDOWS,
  RETRY_BASE_DELAY_MS,
} = require('../run-rust-tests-sequential.cjs') as {
  isTransientWindowsExecFailure: (output: string) => boolean;
  maxAttemptsFor: (platform: NodeJS.Platform) => number;
  runCargoTestWithRetries: (
    cargoArgs: string[],
    targetLabel: string,
    deps?: {
      runCargoTest?: (cargoArgs: string[]) => {
        status: number;
        output: string;
      };
      sleepMs?: (ms: number) => void;
      maxAttempts?: number;
    },
  ) => number;
  writeCapturedOutput: (
    stdout: string,
    stderr: string,
    deps?: {
      writeSync?: (
        fd: number,
        buffer: Buffer,
        offset: number,
        length: number,
        position: number | null,
      ) => number;
      stdoutFd?: number;
      stderrFd?: number;
      sleepMs?: (ms: number) => void;
    },
  ) => void;
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
      expect(isTransientWindowsExecFailure('액세스가 거부되었습니다')).toBe(
        true,
      );
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

  describe('writeCapturedOutput', () => {
    /** >64 KB so a single pipe write cannot accept the whole payload. */
    const CARGO_TAIL =
      'failures:\n    big_suite::the_failing_test\ntest result: FAILED. 0 passed; 1 failed\n';
    const BIG_STDOUT = `${'ok\n'.repeat(20_000)}${CARGO_TAIL}`;

    it('writes stdout and stderr through writeSync before returning', () => {
      const writes: Array<{ fd: number; data: string }> = [];
      const stderr = 'error: test failed, to rerun pass --lib\n';
      writeCapturedOutput(BIG_STDOUT, stderr, {
        writeSync: (fd, buffer, offset, length) => {
          writes.push({
            fd,
            data: buffer.subarray(offset, offset + length).toString(),
          });
          return length;
        },
        stdoutFd: 1,
        stderrFd: 2,
      });
      expect(writes).toEqual([
        { fd: 1, data: BIG_STDOUT },
        { fd: 2, data: stderr },
      ]);
      expect(writes[0]?.data).toContain('failures:');
      expect(writes[0]?.data).toContain('test result: FAILED');
    });

    it('skips empty streams', () => {
      const writes: Array<{ fd: number; data: string }> = [];
      writeCapturedOutput('', 'only-err\n', {
        writeSync: (fd, buffer, offset, length) => {
          writes.push({
            fd,
            data: buffer.subarray(offset, offset + length).toString(),
          });
          return length;
        },
        stdoutFd: 1,
        stderrFd: 2,
      });
      expect(writes).toEqual([{ fd: 2, data: 'only-err\n' }]);
    });

    it('keeps writing after a short write until the payload is complete (#2776)', () => {
      // A pipe that accepts at most one pipe-buffer per call: exactly the
      // CI failure mode where a single fs.writeSync dropped the tail.
      const MAX_CHUNK = 8 * 1024;
      const chunks: string[] = [];
      let calls = 0;
      writeCapturedOutput(BIG_STDOUT, '', {
        writeSync: (fd, buffer, offset, length) => {
          calls += 1;
          const written = Math.min(length, MAX_CHUNK);
          chunks.push(buffer.subarray(offset, offset + written).toString());
          return written;
        },
        stdoutFd: 1,
        stderrFd: 2,
      });
      expect(calls).toBeGreaterThan(1);
      const joined = chunks.join('');
      expect(joined).toBe(BIG_STDOUT);
      expect(joined.endsWith(CARGO_TAIL)).toBe(true);
    });

    it('retries once on EAGAIN and still writes every byte (#2776)', () => {
      const sleeps: number[] = [];
      const written: string[] = [];
      let first = true;
      writeCapturedOutput('hello cargo\n', '', {
        writeSync: (fd, buffer, offset, length) => {
          if (first) {
            first = false;
            throw Object.assign(new Error('resource temporarily unavailable'), {
              code: 'EAGAIN',
            });
          }
          written.push(buffer.subarray(offset, offset + length).toString());
          return length;
        },
        sleepMs: (ms) => sleeps.push(ms),
        stdoutFd: 1,
        stderrFd: 2,
      });
      expect(written.join('')).toBe('hello cargo\n');
      expect(sleeps).toHaveLength(1);
    });

    it('throws instead of spinning forever when the fd takes nothing', () => {
      expect(() =>
        writeCapturedOutput('x', '', {
          writeSync: () => 0,
          sleepMs: () => {},
          stdoutFd: 1,
          stderrFd: 2,
        }),
      ).toThrow(/no progress/);
    });
  });

  describe('piped-output acceptance check (#2776)', () => {
    it('survives process.exit with >64 KB piped stdout and a deliberate failure', () => {
      // Real production path: a child process writes the captured cargo
      // output with writeCapturedOutput, then exits 101 like a failing
      // cargo test run, while stdout is an OS pipe (as in GitHub Actions).
      const tail =
        'failures:\n    big_suite::the_failing_test\ntest result: FAILED. 0 passed; 1 failed\n';
      const driver = `
        const { writeCapturedOutput } = require(${JSON.stringify(RUNNER_PATH)});
        const tail = 'failures:\\n    big_suite::the_failing_test\\ntest result: FAILED. 0 passed; 1 failed\\n';
        writeCapturedOutput('x'.repeat(200_000) + tail, 'captured stderr tail\\n');
        process.exit(101);
      `;
      const child = spawnSync(process.execPath, ['-e', driver], {
        encoding: 'utf8',
        maxBuffer: 16 * 1024 * 1024,
      });
      expect(child.status).toBe(101);
      expect(child.stdout.length).toBeGreaterThan(64 * 1024);
      // The whole payload must be there, tail included — the #2776 symptom
      // was stdout ending mid-line with no `failures:` section.
      expect(child.stdout.endsWith(tail)).toBe(true);
      expect(child.stderr).toContain('captured stderr tail');
    });
  });
});
