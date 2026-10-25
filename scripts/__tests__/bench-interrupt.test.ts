import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import { describe, expect, it } from 'vitest';

const require = createRequire(import.meta.url);
const {
  createInterruptController,
  descendantPids,
  killProcessTree,
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

describe('createInterruptController', () => {
  it('counts one press per call and routes it to the attached handler', () => {
    const controller = createInterruptController();
    const seen: number[] = [];
    controller.press(); // between runs: counted, nothing attached
    controller.detach();
    controller.attach((press: number) => seen.push(press));
    controller.press();
    controller.press();
    expect(seen).toEqual([2, 3]);
    expect(controller.presses).toBe(3);
  });

  it('first press does not kill, second press kills the child tree', () => {
    const kills: number[] = [];
    const controller = createInterruptController();
    controller.attach((press: number) => {
      if (press >= 2) {
        killProcessTree(
          4242,
          (pid: number) => kills.push(pid),
          '/nonexistent-proc',
        );
      }
    });
    controller.press();
    expect(kills).toEqual([]);
    controller.press();
    expect(kills).toEqual([4242]);
  });
});

describe.skipIf(process.platform !== 'linux')('real SIGINT delivery', () => {
  it('one SIGINT is one press, two are two', async () => {
    const lib = require.resolve('../lib/bench-interrupt.cjs');
    const script = `
      const { createInterruptController } = require(${JSON.stringify(lib)});
      const c = createInterruptController();
      c.attach((n) => console.log('press ' + n));
      process.on('SIGINT', () => c.press());
      console.log('ready');
      setTimeout(() => process.exit(0), 3000);
    `;
    const child = spawn(process.execPath, ['-e', script]);
    let out = '';
    child.stdout.on('data', (d) => (out += d));
    await new Promise<void>((resolve) => {
      const wait = setInterval(() => {
        if (out.includes('ready')) {
          clearInterval(wait);
          resolve();
        }
      }, 20);
    });
    child.kill('SIGINT');
    await new Promise((r) => setTimeout(r, 200));
    child.kill('SIGINT');
    await new Promise((r) => setTimeout(r, 200));
    child.kill('SIGKILL');
    expect(out.trim().split('\n')).toEqual(['ready', 'press 1', 'press 2']);
  });
});
