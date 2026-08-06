import { describe, expect, it } from 'vitest';
import {
  filterVisibleProcesses,
  processListFingerprint,
  summarizeVisibleProcesses,
} from '../listProcessesShared';
import type { ProcessEntry } from '../types';

function entry(
  partial: Pick<ProcessEntry, 'process_id' | 'status'> &
    Partial<ProcessEntry>,
): ProcessEntry {
  return {
    process_id: partial.process_id,
    name: partial.name ?? null,
    command: partial.command ?? 'echo',
    status: partial.status,
    pid: partial.pid ?? 1,
    started_at: partial.started_at ?? '2026-08-06T00:00:00.000Z',
    exit_code: partial.exit_code ?? null,
  };
}

describe('listProcessesShared', () => {
  it('filters killed processes from the visible list', () => {
    const visible = filterVisibleProcesses([
      entry({ process_id: 'a', status: 'running' }),
      entry({ process_id: 'b', status: 'killed' }),
      entry({ process_id: 'c', status: 'finished' }),
    ]);
    expect(visible.map((item) => item.process_id)).toEqual(['a', 'c']);
  });

  it('derives summary counts from the visible list only', () => {
    const summary = summarizeVisibleProcesses([
      entry({ process_id: 'a', status: 'running' }),
      entry({ process_id: 'b', status: 'starting' }),
      entry({ process_id: 'c', status: 'finished' }),
      entry({ process_id: 'd', status: 'failed' }),
    ]);
    expect(summary).toEqual({ total: 4, running: 2, finished: 2 });
  });

  it('builds a stable fingerprint independent of input order', () => {
    const a = processListFingerprint([
      { process_id: 'b', status: 'finished' },
      { process_id: 'a', status: 'running' },
    ]);
    const b = processListFingerprint([
      { process_id: 'a', status: 'running' },
      { process_id: 'b', status: 'finished' },
    ]);
    expect(a).toBe(b);
    expect(a).toBe('a:running|b:finished');
  });
});
