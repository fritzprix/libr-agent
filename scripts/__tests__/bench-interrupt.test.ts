import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import { describe, expect, it } from 'vitest';

const require = createRequire(import.meta.url);
const {
  descendantPids,
  leftoverTaskContainers,
} = require('../lib/bench-interrupt.cjs');

describe('leftoverTaskContainers', () => {
  it('returns only compose containers that started after the snapshot', () => {
    expect(
      leftoverTaskContainers(
        ['old1'],
        [
          { id: 'old1', composeProject: 'harbor-old' },
          { id: 'new1', composeProject: 'task__abc' },
          { id: 'new2', composeProject: '' },
        ],
      ),
    ).toEqual(['new1']);
  });
});

describe.skipIf(process.platform !== 'linux')('descendantPids', () => {
  it('finds nested children of a process and nothing else', async () => {
    const parent = spawn('bash', ['-c', 'bash -c "sleep 30; true" & wait'], {
      stdio: 'ignore',
    });
    try {
      await new Promise((resolve) => setTimeout(resolve, 300));
      const pids: number[] = descendantPids(parent.pid as number);
      expect(pids.length).toBeGreaterThanOrEqual(2);
      expect(pids).not.toContain(parent.pid);
      expect(pids).not.toContain(process.pid);
      for (const pid of pids) process.kill(pid, 'SIGKILL');
    } finally {
      parent.kill('SIGKILL');
    }
  });

  it('returns an empty list when /proc is unavailable', () => {
    expect(descendantPids(1, '/nonexistent-proc')).toEqual([]);
  });
});
