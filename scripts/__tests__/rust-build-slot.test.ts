import { spawn, spawnSync } from 'node:child_process';
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  utimesSync,
  writeFileSync,
} from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { afterEach, beforeAll, beforeEach, describe, expect, it } from 'vitest';

const require = createRequire(import.meta.url);
const { maxConcurrent, releaseBuildSlot, tryAcquire } =
  require('../rust-build-slot.cjs') as {
    maxConcurrent: (env: NodeJS.ProcessEnv, totalMem: number) => number;
    releaseBuildSlot: (slot: { file: string; token: string }) => void;
    tryAcquire: (
      dir: string,
      limit: number,
      label: string,
      pid?: number,
    ) => { file: string; token: string } | null;
  };

const GIB = 1024 ** 3;
const runner = path.resolve(__dirname, '..', 'run-rust-command.cjs');

/** A PID that has already exited: run a process to completion and keep its PID. */
function deadPid(): number {
  return spawnSync(process.execPath, ['-e', '0']).pid;
}

describe('rust build slot', () => {
  let dir: string;

  // Windows shells out to PowerShell for process start times. Pay that once
  // here so the 5s default test timeout is not racing a cold runner.
  beforeAll(() => {
    const warmupDir = mkdtempSync(path.join(tmpdir(), 'rust-slot-warmup-'));
    try {
      const held = tryAcquire(warmupDir, 1, 'warmup');
      if (held) {
        releaseBuildSlot(held);
      }
    } finally {
      rmSync(warmupDir, { recursive: true, force: true });
    }
  }, 30000);

  beforeEach(() => {
    dir = mkdtempSync(path.join(tmpdir(), 'rust-slot-'));
  });

  afterEach(() => {
    rmSync(dir, { recursive: true, force: true });
  });

  it('allows one build per 32GB of installed RAM, at least one', () => {
    expect(maxConcurrent({}, 16 * GIB)).toBe(1);
    expect(maxConcurrent({}, 31 * GIB)).toBe(1);
    expect(maxConcurrent({}, 59 * GIB)).toBe(1);
    // The OS reports less than the installed amount.
    expect(maxConcurrent({}, 62.7 * GIB)).toBe(2);
    expect(maxConcurrent({}, 64 * GIB)).toBe(2);
    expect(maxConcurrent({}, 94 * GIB)).toBe(3);
    expect(maxConcurrent({}, 125 * GIB)).toBe(4);
    expect(
      maxConcurrent({ LIBRAGENT_RUST_MAX_CONCURRENT: '3' }, 16 * GIB),
    ).toBe(3);
    expect(
      maxConcurrent({ LIBRAGENT_RUST_MAX_CONCURRENT: 'x' }, 16 * GIB),
    ).toBe(1);
  });

  it(
    'refuses a slot while a live process holds it',
    () => {
      const held = tryAcquire(dir, 1, 'a');
      expect(held).not.toBeNull();
      expect(tryAcquire(dir, 1, 'b')).toBeNull();
      releaseBuildSlot(held!);
      expect(tryAcquire(dir, 1, 'b')).not.toBeNull();
    },
    15000,
  );

  it('takes a slot from a dead holder', () => {
    const dead = tryAcquire(dir, 1, 'crashed', deadPid());
    expect(dead).not.toBeNull();
    const next = tryAcquire(dir, 1, 'next');
    expect(next).not.toBeNull();
    expect(next!.token).not.toBe(dead!.token);
  });

  it('takes a slot whose holder is far too old to be a build', () => {
    mkdirSync(dir, { recursive: true });
    const ancient = Date.now() - 7 * 60 * 60 * 1000;
    writeFileSync(
      path.join(dir, 'slot-0.json'),
      JSON.stringify({ pid: process.pid, token: 'reused-pid', since: ancient }),
    );
    expect(tryAcquire(dir, 1, 'next')).not.toBeNull();
  });

  it('takes a slot whose live PID now belongs to another process', () => {
    mkdirSync(dir, { recursive: true });
    writeFileSync(
      path.join(dir, 'slot-0.json'),
      JSON.stringify({
        pid: process.pid,
        pidStart: 'an-earlier-process',
        token: 'reused-pid',
        since: Date.now(),
      }),
    );
    expect(tryAcquire(dir, 1, 'next')).not.toBeNull();
  });

  it('keeps a slot whose holder start time matches', () => {
    const held = tryAcquire(dir, 1, 'a');
    const record = JSON.parse(readFileSync(held!.file, 'utf8')) as {
      pidStart: string | null;
    };
    expect(record.pidStart).toBeTruthy();
    expect(tryAcquire(dir, 1, 'b')).toBeNull();
  });

  it('leaves a stale slot alone while another contender reclaims it', () => {
    const dead = tryAcquire(dir, 1, 'crashed', deadPid());
    writeFileSync(`${dead!.file}.guard`, '');
    expect(tryAcquire(dir, 1, 'next')).toBeNull();
    expect(readFileSync(dead!.file, 'utf8')).toContain(dead!.token);
  });

  it('recovers from a guard left by a crash', () => {
    const dead = tryAcquire(dir, 1, 'crashed', deadPid());
    const guard = `${dead!.file}.guard`;
    writeFileSync(guard, '');
    const old = new Date(Date.now() - 60000);
    utimesSync(guard, old, old);
    expect(tryAcquire(dir, 1, 'next')).toBeNull();
    expect(tryAcquire(dir, 1, 'next')).not.toBeNull();
    expect(readdirSync(dir)).toEqual(['slot-0.json']);
  });

  it('does not let a late release remove the next holder', () => {
    const dead = tryAcquire(dir, 1, 'crashed', deadPid());
    const next = tryAcquire(dir, 1, 'next');
    releaseBuildSlot(dead!);
    expect(tryAcquire(dir, 1, 'other')).toBeNull();
    releaseBuildSlot(next!);
    expect(tryAcquire(dir, 1, 'other')).not.toBeNull();
  });

  it('leaves no temporary files behind', () => {
    const held = tryAcquire(dir, 2, 'a');
    tryAcquire(dir, 2, 'b');
    tryAcquire(dir, 2, 'c');
    releaseBuildSlot(held!);
    expect(
      readdirSync(dir).filter((name) => !name.startsWith('slot-')),
    ).toEqual([]);
  });

  it.skipIf(process.platform === 'win32')(
    'serializes cargo runs from separate processes',
    async () => {
      // A fake cargo appends begin/end lines, so overlap shows up in the log.
      const bin = path.join(dir, 'bin');
      mkdirSync(bin);
      const log = path.join(dir, 'log');
      const cargo = path.join(bin, 'cargo');
      writeFileSync(
        cargo,
        `#!/bin/sh\necho begin >> "${log}"\nsleep 0.3\necho end >> "${log}"\n`,
      );
      chmodSync(cargo, 0o755);
      const env = {
        ...process.env,
        PATH: `${bin}${path.delimiter}${process.env.PATH}`,
        LIBRAGENT_RUST_SLOT_DIR: path.join(dir, 'slots'),
        LIBRAGENT_RUST_MAX_CONCURRENT: '1',
      };
      const codes = await Promise.all(
        [0, 1, 2].map(
          () =>
            new Promise<number | null>((resolve) => {
              spawn(process.execPath, [runner, 'check'], {
                env,
                stdio: 'ignore',
              }).on('exit', resolve);
            }),
        ),
      );
      expect(codes).toEqual([0, 0, 0]);
      const lines = readFileSync(log, 'utf8').trim().split('\n');
      expect(lines).toEqual(['begin', 'end', 'begin', 'end', 'begin', 'end']);
    },
    30000,
  );
});
