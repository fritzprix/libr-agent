'use strict';
/**
 * Helpers for stopping an isolated Harbor run on Ctrl+C (see run-harbor-bench.cjs).
 */

const fs = require('node:fs');
const path = require('node:path');

/**
 * Every descendant of `rootPid`, found through /proc/<pid>/stat parent links.
 * Returns an empty list where /proc is unavailable.
 * @param {number} rootPid
 * @param {string} [procRoot]
 * @returns {number[]}
 */
function descendantPids(rootPid, procRoot = '/proc') {
  /** @type {Map<number, number[]>} */
  const children = new Map();
  let entries;
  try {
    entries = fs.readdirSync(procRoot);
  } catch {
    return [];
  }
  for (const entry of entries) {
    if (!/^\d+$/.test(entry)) continue;
    let stat;
    try {
      stat = fs.readFileSync(path.join(procRoot, entry, 'stat'), 'utf8');
    } catch {
      continue; // exited while scanning
    }
    // "pid (comm) state ppid ..." where comm may contain spaces and parentheses.
    const afterComm = stat.slice(stat.lastIndexOf(')') + 2).split(' ');
    const ppid = Number(afterComm[1]);
    if (!Number.isInteger(ppid)) continue;
    const siblings = children.get(ppid) ?? [];
    siblings.push(Number(entry));
    children.set(ppid, siblings);
  }

  /** @type {number[]} */
  const found = [];
  const queue = [rootPid];
  while (queue.length > 0) {
    const pid = /** @type {number} */ (queue.shift());
    for (const child of children.get(pid) ?? []) {
      found.push(child);
      queue.push(child);
    }
  }
  return found;
}

/**
 * Containers that appeared during the run: running now, absent from the snapshot
 * taken before it started. Only compose-managed ones (Harbor task containers) count.
 * @param {string[]} before container ids from `docker ps -q` before the run
 * @param {{ id: string, composeProject: string }[]} now running containers
 * @returns {string[]}
 */
function leftoverTaskContainers(before, now) {
  const known = new Set(before);
  return now
    .filter((container) => !known.has(container.id) && container.composeProject)
    .map((container) => container.id);
}

module.exports = { descendantPids, leftoverTaskContainers };
