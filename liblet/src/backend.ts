/**
 * Finding a running shared backend.
 *
 * Every backend publishes `<db file>.backend.<pid>.json` next to its
 * database (`src-tauri/src/server/backend_descriptor.rs`). The TUI reads
 * those records — it never takes a lock and never starts a hidden GUI.
 */

import { promises as fs } from 'node:fs';
import * as net from 'node:net';
import * as os from 'node:os';
import * as path from 'node:path';

import type { HealthStatus } from './types.js';

export type Profile = 'prod' | 'dev' | 'demo';

export interface BackendRecord {
  pid: number;
  port: number;
  startedAtMs: number;
}

export class NoBackendError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'NoBackendError';
  }
}

export function resolveProfile(env: NodeJS.ProcessEnv = process.env): Profile {
  const raw = (env.LIBRAGENT_PROFILE ?? '').toLowerCase();
  return raw === 'dev' || raw === 'demo' ? raw : 'prod';
}

/**
 * Same resolution order as `src-tauri/src/profile.rs`:
 * `LIBRAGENT_DATA_DIR`, then the platform data dir + the profile folder.
 */
export function dataDirFor(
  profile: Profile,
  env: NodeJS.ProcessEnv = process.env,
  platform: NodeJS.Platform = process.platform,
  envOrSystem: NodeJS.ProcessEnv = process.env,
): string {
  if (env.LIBRAGENT_DATA_DIR) return env.LIBRAGENT_DATA_DIR;
  const folder =
    profile === 'demo'
      ? 'com.fritzprix.libragent-demo'
      : 'com.fritzprix.libragent';
  let base: string;
  if (platform === 'win32') {
    base = envOrSystem.APPDATA ?? path.join(os.homedir(), 'AppData', 'Roaming');
  } else if (platform === 'darwin') {
    base = path.join(os.homedir(), 'Library', 'Application Support');
  } else {
    base =
      envOrSystem.XDG_DATA_HOME ?? path.join(os.homedir(), '.local', 'share');
  }
  return path.join(base, folder);
}

export function dbFileFor(profile: Profile): string {
  switch (profile) {
    case 'dev':
      return 'libragent_v2.dev.db';
    case 'demo':
      return 'libragent_v2.demo.db';
    default:
      return 'libragent_v2.db';
  }
}

/** Loopback connect probe, mirroring the backend's own 300 ms check. */
export function portIsOpen(port: number, timeoutMs = 300): Promise<boolean> {
  return new Promise((resolve) => {
    const socket = net.connect({ host: '127.0.0.1', port });
    const done = (open: boolean) => {
      socket.destroy();
      resolve(open);
    };
    socket.setTimeout(timeoutMs);
    socket.once('connect', () => done(true));
    socket.once('timeout', () => done(false));
    socket.once('error', () => done(false));
  });
}

/** Read all live backend records for one database, freshest first. */
export async function discoverBackends(
  dbPath: string,
  probe: (port: number) => Promise<boolean> = portIsOpen,
): Promise<BackendRecord[]> {
  const dir = path.dirname(dbPath);
  const prefix = `${path.basename(dbPath)}.backend.`;
  let entries: string[];
  try {
    entries = await fs.readdir(dir);
  } catch {
    return [];
  }
  const records: BackendRecord[] = [];
  for (const entry of entries) {
    if (!entry.startsWith(prefix) || !entry.endsWith('.json')) continue;
    try {
      const raw = await fs.readFile(path.join(dir, entry), 'utf8');
      const parsed = JSON.parse(raw) as Partial<BackendRecord>;
      if (
        typeof parsed.pid === 'number' &&
        typeof parsed.port === 'number' &&
        (await probe(parsed.port))
      ) {
        records.push({
          pid: parsed.pid,
          port: parsed.port,
          startedAtMs: parsed.startedAtMs ?? 0,
        });
      }
    } catch {
      // A torn or foreign record is simply not a live backend.
    }
  }
  return records.sort((a, b) => b.startedAtMs - a.startedAtMs);
}

export async function probeHealth(
  port: number,
  fetchImpl: typeof fetch = fetch,
  timeoutMs = 2000,
): Promise<HealthStatus | null> {
  try {
    const res = await fetchImpl(`http://127.0.0.1:${port}/api/health`, {
      signal: AbortSignal.timeout(timeoutMs),
    });
    if (!res.ok) return null;
    return (await res.json()) as HealthStatus;
  } catch {
    return null;
  }
}

export interface BackendLocation {
  port: number;
  health: HealthStatus;
}

/**
 * The base URL of a ready shared backend, or `NoBackendError`.
 * Ready means `GET /api/health` answers `ready: true` (startup session
 * recovery may still be running behind a bare HTTP 200).
 */
export async function findReadyBackend(
  env: NodeJS.ProcessEnv = process.env,
  fetchImpl: typeof fetch = fetch,
): Promise<BackendLocation> {
  const profile = resolveProfile(env);
  const dbPath =
    env.LIBRAGENT_DB_PATH ??
    path.join(dataDirFor(profile, env), dbFileFor(profile));
  const records = await discoverBackends(dbPath);
  for (const record of records) {
    const health = await probeHealth(record.port, fetchImpl);
    if (health?.ready) return { port: record.port, health };
  }
  throw new NoBackendError(
    `No ready LibrAgent backend found for the ${profile} profile. ` +
      'Start the LibrAgent app (the GUI starts the shared backend).',
  );
}
