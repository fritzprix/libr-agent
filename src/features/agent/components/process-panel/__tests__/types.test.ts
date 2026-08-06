import { describe, expect, it } from 'vitest';
import {
  isActiveProcessStatus,
  parseListProcessesResult,
  parseReadProcessOutputResult,
} from '../types';

describe('process-panel types', () => {
  it('parses a valid listProcesses payload', () => {
    const parsed = parseListProcessesResult({
      processes: [
        {
          process_id: 'proc-1',
          name: 'build',
          command: 'pnpm build',
          status: 'running',
          pid: 4242,
          started_at: '2026-08-06T00:00:00Z',
          exit_code: null,
        },
      ],
      total: 1,
      running: 1,
      finished: 0,
    });

    expect(parsed).toEqual({
      processes: [
        {
          process_id: 'proc-1',
          name: 'build',
          command: 'pnpm build',
          status: 'running',
          pid: 4242,
          started_at: '2026-08-06T00:00:00Z',
          exit_code: null,
        },
      ],
      total: 1,
      running: 1,
      finished: 0,
    });
  });

  it('accepts nullish pid and exit_code', () => {
    const parsed = parseListProcessesResult({
      processes: [
        {
          process_id: 'proc-2',
          command: 'echo hi',
          status: 'finished',
          started_at: '2026-08-06T00:00:00Z',
        },
      ],
      total: 1,
      running: 0,
      finished: 1,
    });

    expect(parsed?.processes[0]?.pid).toBeUndefined();
    expect(parsed?.processes[0]?.exit_code).toBeUndefined();
  });

  it('maps unknown process status to finished instead of rejecting the list', () => {
    const parsed = parseListProcessesResult({
      processes: [
        {
          process_id: 'proc-x',
          command: 'echo',
          status: 'weird',
          started_at: '2026-08-06T00:00:00Z',
          pid: null,
          exit_code: null,
        },
      ],
      total: 1,
      running: 0,
      finished: 0,
    });

    expect(parsed?.processes[0]?.status).toBe('finished');
  });

  it('rejects invalid listProcesses payloads', () => {
    expect(parseListProcessesResult({ processes: 'nope' })).toBeNull();
    expect(
      parseListProcessesResult({
        processes: [{ process_id: 'x', status: 'running' }],
        total: 1,
        running: 0,
        finished: 0,
      }),
    ).toBeNull();
  });

  it('parses readProcessOutput structured content', () => {
    const parsed = parseReadProcessOutputResult({
      process_id: 'proc-1',
      stream: 'both',
      mode: 'tail',
      status: 'finished',
      is_process_running: false,
      outputs: {
        stdout: { content: ['hello'], lines_returned: 1 },
        stderr: { content: [] },
      },
    });

    expect(parsed?.outputs.stdout?.content).toEqual(['hello']);
    expect(parsed?.outputs.stderr?.content).toEqual([]);
  });

  it('falls back when output stream content is missing', () => {
    const parsed = parseReadProcessOutputResult({
      process_id: 'proc-1',
      stream: 'both',
      mode: 'tail',
      status: 'finished',
      outputs: {
        stdout: {},
      },
    });

    expect(parsed?.outputs.stdout?.content).toEqual([]);
  });

  it('identifies active process statuses', () => {
    expect(isActiveProcessStatus('starting')).toBe(true);
    expect(isActiveProcessStatus('running')).toBe(true);
    expect(isActiveProcessStatus('finished')).toBe(false);
    expect(isActiveProcessStatus('failed')).toBe(false);
    expect(isActiveProcessStatus('killed')).toBe(false);
  });
});
