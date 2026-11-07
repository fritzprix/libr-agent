/**
 * Machine-wide limit on concurrent Rust builds.
 *
 * Per-process job limits do not help when several worktrees build at once: each
 * Tauri test binary links at ~600MB+, and two links exhaust a 32GB machine. Every
 * cargo run through run-rust-command.cjs holds one of these slots, whichever
 * checkout it comes from. Waiting has no timeout: a build behind another build is
 * normal, and a holder that dies frees its slot.
 *
 * LIBRAGENT_RUST_MAX_CONCURRENT overrides the limit (default: one per 32GB of RAM,
 * at least one). LIBRAGENT_RUST_SLOT_DIR overrides the directory (tests).
 */

const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const POLL_MS = 1000;
const REPORT_EVERY_MS = 60000;
const GIB = 1024 ** 3;
// No cargo run takes this long; a holder this old is a reused PID, not a build.
const MAX_HOLD_MS = 6 * 60 * 60 * 1000;

function slotDir(env = process.env) {
  if (env.LIBRAGENT_RUST_SLOT_DIR) {
    return env.LIBRAGENT_RUST_SLOT_DIR;
  }
  const user = typeof process.getuid === 'function' ? process.getuid() : 'user';
  return path.join(os.tmpdir(), `libragent-rust-build-slots-${user}`);
}

function maxConcurrent(env = process.env, totalMem = os.totalmem()) {
  const configured = Number.parseInt(
    env.LIBRAGENT_RUST_MAX_CONCURRENT ?? '',
    10,
  );
  if (Number.isInteger(configured) && configured > 0) {
    return configured;
  }
  return Math.max(1, Math.floor(Math.round(totalMem / GIB) / 32));
}

function pidAlive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    // EPERM: the process exists but belongs to someone else.
    return error.code === 'EPERM';
  }
}

function readHolder(file) {
  try {
    return JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch {
    return null;
  }
}

/**
 * Remove a slot file only if it still holds the content judged stale. Rename is
 * atomic and only one contender wins it; a contender that moved a newer holder's
 * file by mistake puts it straight back.
 */
function reclaimIfStale(file) {
  const raw = (() => {
    try {
      return fs.readFileSync(file, 'utf8');
    } catch {
      return null;
    }
  })();
  if (raw === null) {
    return;
  }
  const holder = readHolder(file);
  if (
    holder &&
    pidAlive(holder.pid) &&
    Date.now() - holder.since < MAX_HOLD_MS
  ) {
    return;
  }
  const moved = `${file}.reclaim.${crypto.randomUUID()}`;
  try {
    fs.renameSync(file, moved);
  } catch {
    return;
  }
  if (fs.readFileSync(moved, 'utf8') !== raw) {
    try {
      fs.linkSync(moved, file);
    } catch {
      // Another run took the slot meanwhile; the moved holder is already replaced.
    }
  }
  fs.unlinkSync(moved);
}

/** Try every slot once. Returns a held slot or null. */
function tryAcquire(dir, limit, label, pid = process.pid) {
  fs.mkdirSync(dir, { recursive: true });
  const token = crypto.randomUUID();
  const content = JSON.stringify({ pid, token, label, since: Date.now() });
  const temp = path.join(dir, `.pending-${token}`);
  fs.writeFileSync(temp, content);
  try {
    for (let i = 0; i < limit; i += 1) {
      const file = path.join(dir, `slot-${i}.json`);
      reclaimIfStale(file);
      try {
        // A link is atomic, exclusive, and never shows a partial payload.
        fs.linkSync(temp, file);
        return { file, token };
      } catch (error) {
        if (error.code !== 'EEXIST') {
          throw error;
        }
      }
    }
    return null;
  } finally {
    fs.unlinkSync(temp);
  }
}

function sleepMs(ms) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

function acquireBuildSlot(label, env = process.env) {
  const dir = slotDir(env);
  const limit = maxConcurrent(env);
  let lastReport = 0;
  for (;;) {
    const slot = tryAcquire(dir, limit, label);
    if (slot) {
      return slot;
    }
    if (Date.now() - lastReport >= REPORT_EVERY_MS) {
      lastReport = Date.now();
      console.error(
        `[rust-build-slot] waiting: ${limit} concurrent Rust build(s) allowed on this machine (${dir})`,
      );
    }
    sleepMs(POLL_MS);
  }
}

/** Only the holder's token removes the file, so a late release frees nothing else. */
function releaseBuildSlot(slot) {
  const holder = readHolder(slot.file);
  if (holder && holder.token === slot.token) {
    fs.rmSync(slot.file, { force: true });
  }
}

module.exports = {
  acquireBuildSlot,
  maxConcurrent,
  releaseBuildSlot,
  slotDir,
  tryAcquire,
};
