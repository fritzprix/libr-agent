/**
 * Machine-wide limit on concurrent Rust builds.
 *
 * Per-process job limits do not help when several worktrees build at once: each
 * Tauri test binary links at ~600MB+, and two links exhaust a 32GB machine. Every
 * cargo run through run-rust-command.cjs holds one of these slots, whichever
 * checkout it comes from. Waiting has no timeout: a build behind another build is
 * normal, and a holder that dies frees its slot.
 *
 * LIBRAGENT_RUST_MAX_CONCURRENT overrides the limit (default: one per 32GB of
 * installed RAM, at least one). LIBRAGENT_RUST_SLOT_DIR overrides the directory (tests).
 */

const { spawnSync } = require('node:child_process');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const POLL_MS = 1000;
const REPORT_EVERY_MS = 60000;
const GIB = 1024 ** 3;
const REPORTED_RAM_SHORTFALL_GIB = 4;
// No cargo run takes this long. The last resort for a PID whose start time cannot be read.
const MAX_HOLD_MS = 6 * 60 * 60 * 1000;
// Reading a start time spawns a process outside Linux, and waiters poll every second.
const IDENTITY_RECHECK_MS = 30000;
const GUARD_STALE_MS = 30000;
// PowerShell cold-start on Windows CI can take several seconds; never hang forever.
const PID_START_TIMEOUT_MS = 15000;

const identityCheckedAt = new Map();
let ownPidStart;

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
  // The OS reports less than the installed RAM (64GB shows as about 62.7GiB).
  return Math.max(1, Math.floor((totalMem / GIB + REPORTED_RAM_SHORTFALL_GIB) / 32));
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

/** A value that changes when `pid` is reused; null where it cannot be read. */
function pidStart(pid) {
  if (process.platform === 'linux') {
    try {
      const stat = fs.readFileSync(`/proc/${pid}/stat`, 'utf8');
      // The comm field may contain spaces. starttime is field 22 of the file.
      return stat.slice(stat.lastIndexOf(')') + 2).split(' ')[19] ?? null;
    } catch {
      return null;
    }
  }
  const [command, args] =
    process.platform === 'win32'
      ? [
          'powershell.exe',
          [
            '-NoProfile',
            '-NonInteractive',
            '-Command',
            `(Get-Process -Id ${pid}).StartTime.ToFileTimeUtc()`,
          ],
        ]
      : ['ps', ['-o', 'lstart=', '-p', String(pid)]];
  const result = spawnSync(command, args, {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'ignore'],
    windowsHide: true,
    timeout: PID_START_TIMEOUT_MS,
  });
  return result.status === 0 ? result.stdout.trim() || null : null;
}

/** Cache this process's start time so Windows does not spawn PowerShell twice. */
function startTimeFor(pid) {
  if (pid === process.pid) {
    if (ownPidStart === undefined) {
      ownPidStart = pidStart(pid);
    }
    return ownPidStart;
  }
  return pidStart(pid);
}

function pidWasReused(holder) {
  if (!holder.pidStart) {
    return false;
  }
  const checkedAt = identityCheckedAt.get(holder.token);
  if (checkedAt && Date.now() - checkedAt < IDENTITY_RECHECK_MS) {
    return false;
  }
  const current = startTimeFor(holder.pid);
  if (current !== null && current !== holder.pidStart) {
    return true;
  }
  identityCheckedAt.set(holder.token, Date.now());
  return false;
}

function holderIsStale(holder) {
  return (
    !holder ||
    !pidAlive(holder.pid) ||
    Date.now() - holder.since >= MAX_HOLD_MS ||
    pidWasReused(holder)
  );
}

/**
 * Remove a stale slot file. Only one contender works on a slot at a time and it
 * re-reads the holder under the guard, so it cannot remove a newer holder that
 * another contender's reclaim made room for. A guard left by a crash is removed
 * after GUARD_STALE_MS; this contender then retries on its next poll.
 */
function reclaimIfStale(file) {
  if (!fs.existsSync(file) || !holderIsStale(readHolder(file))) {
    return;
  }
  const guard = `${file}.guard`;
  let fd;
  try {
    fd = fs.openSync(guard, 'wx');
  } catch (error) {
    if (error.code !== 'EEXIST') {
      throw error;
    }
    try {
      if (Date.now() - fs.statSync(guard).mtimeMs > GUARD_STALE_MS) {
        fs.rmSync(guard, { force: true });
      }
    } catch {
      // The guard's owner finished in the meantime.
    }
    return;
  }
  try {
    if (fs.existsSync(file) && holderIsStale(readHolder(file))) {
      fs.rmSync(file, { force: true });
    }
  } finally {
    fs.closeSync(fd);
    fs.rmSync(guard, { force: true });
  }
}

/** Try every slot once. Returns a held slot or null. */
function tryAcquire(dir, limit, label, pid = process.pid) {
  fs.mkdirSync(dir, { recursive: true });
  const token = crypto.randomUUID();
  const content = JSON.stringify({
    pid,
    pidStart: startTimeFor(pid),
    token,
    label,
    since: Date.now(),
  });
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
