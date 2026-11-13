import { mkdtemp, writeFile } from 'node:fs/promises';
import * as os from 'node:os';
import * as path from 'node:path';

import { describe, expect, it } from 'vitest';

import {
  dataDirFor,
  dbFileFor,
  discoverBackends,
  resolveProfile,
} from '../src/backend.js';

describe('profile and paths (mirrors src-tauri/src/profile.rs)', () => {
  it('resolves the profile from the environment', () => {
    expect(resolveProfile({})).toBe('prod');
    expect(resolveProfile({ LIBRAGENT_PROFILE: 'dev' })).toBe('dev');
    expect(resolveProfile({ LIBRAGENT_PROFILE: 'nonsense' })).toBe('prod');
  });

  it('names the per-profile database file', () => {
    expect(dbFileFor('prod')).toBe('libragent_v2.db');
    expect(dbFileFor('dev')).toBe('libragent_v2.dev.db');
    expect(dbFileFor('demo')).toBe('libragent_v2.demo.db');
  });

  it('uses the platform data dir and the profile folder', () => {
    expect(
      dataDirFor(
        'prod',
        { APPDATA: 'C:\\Users\\x\\AppData\\Roaming' },
        'win32',
      ),
    ).toBe(
      path.join('C:\\Users\\x\\AppData\\Roaming', 'com.fritzprix.libragent'),
    );
    expect(
      dataDirFor('demo', { XDG_DATA_HOME: '/home/x/.local/share' }, 'linux'),
    ).toBe(path.join('/home/x/.local/share', 'com.fritzprix.libragent-demo'));
  });

  it('LIBRAGENT_DATA_DIR wins over everything', () => {
    expect(dataDirFor('prod', { LIBRAGENT_DATA_DIR: '/tmp/here' })).toBe(
      '/tmp/here',
    );
  });
});

describe('backend record discovery', () => {
  it('finds live records only, freshest first', async () => {
    const dir = await mkdtemp(path.join(os.tmpdir(), 'liblet-test-'));
    const dbPath = path.join(dir, 'libragent_v2.db');
    const write = async (pid: number, port: number, started: number) =>
      writeFile(
        path.join(dir, `libragent_v2.db.backend.${pid}.json`),
        JSON.stringify({ pid, port, startedAtMs: started }),
      );
    await write(100, 7000, 1);
    await write(200, 7001, 5);
    await writeFile(
      path.join(dir, 'libragent_v2.db.backend.300.json'),
      'not json',
    );

    const openPorts = new Set([7000, 7001]);
    const records = await discoverBackends(dbPath, async (port) =>
      openPorts.has(port),
    );
    expect(records.map((r) => r.pid)).toEqual([200, 100]);

    const none = await discoverBackends(dbPath, async () => false);
    expect(none).toEqual([]);

    const missing = await discoverBackends(
      path.join(dir, 'nope', 'libragent_v2.db'),
    );
    expect(missing).toEqual([]);
  });
});
