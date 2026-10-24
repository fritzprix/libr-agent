#!/usr/bin/env node
/**
 * Sync the `@fre4x/*` entries of mcp-server.json from b1te's `presets.json`
 * (published with each b1te release, fre4x/b1te#584).
 *
 * Rules (#2179):
 * - only entries whose args name an `@fre4x/<pkg>` package are touched; every other
 *   preset is left byte-for-byte as it was
 * - upstream pins `@fre4x/<pkg>@<version>`; commands are recorded as `@latest` (#2178),
 *   so a release that only bumps version numbers changes nothing
 * - a curated `logo` is kept when upstream does not ship one
 * - an upstream name that collides with a non-fre4x preset is skipped
 * - upstream entries are validated (launcher must be `npx -y @fre4x/<pkg>[@version]`,
 *   known fields and shapes only); an invalid entry is rejected, reported, and the
 *   existing local entry is kept
 * - the file is rewritten only when the result differs (idempotent, so no PR)
 *
 * Usage: node scripts/sync-fre4x-presets.cjs --input presets.json
 *          [--target mcp-server.json] [--summary summary.md] [--check]
 * Exit 0 always on success; "changed=true|false" is printed for CI.
 */
const fs = require('node:fs');
const path = require('node:path');

const FIELD_ORDER = [
  'category',
  'description',
  'logo',
  'command',
  'args',
  'env',
  'variableDefinitions',
];

function isFre4x(entry) {
  return (
    Array.isArray(entry?.args) &&
    entry.args.some(
      (arg) => typeof arg === 'string' && arg.startsWith('@fre4x/'),
    )
  );
}

/** `@fre4x/arxiv@1.1.29` -> `@fre4x/arxiv@latest`; other args are untouched. */
function toLatest(arg) {
  const match = /^(@fre4x\/[^@/\s]+)(?:@.+)?$/.exec(arg);
  return match ? `${match[1]}@latest` : arg;
}

function orderFields(entry) {
  const ordered = {};
  for (const key of FIELD_ORDER) {
    if (entry[key] !== undefined) ordered[key] = entry[key];
  }
  for (const key of Object.keys(entry)) {
    if (!(key in ordered)) ordered[key] = entry[key];
  }
  return ordered;
}

const NAME_RE = /^[a-z0-9][a-z0-9-]*$/;
const PACKAGE_ARG_RE = /^@fre4x\/[a-z0-9][a-z0-9-]*(?:@[A-Za-z0-9._+-]+)?$/;
const ENV_KEY_RE = /^[A-Z][A-Z0-9_]*$/;
// Variables that change how the launcher itself runs (registry, proxy, preload,
// trust store, search path): never accepted from upstream, declared or not.
const PROCESS_ENV_RE =
  /^(NODE_OPTIONS|NODE_EXTRA_CA_CERTS|NODE_PATH|PATH|HOME|SHELL|NPM_CONFIG_.*|LD_.*|DYLD_.*|.*_PROXY|SSL_CERT_.*)$/i;
const ALLOWED_FIELDS = new Set(FIELD_ORDER);
const ALLOWED_VARIABLE_FIELDS = new Set([
  'required',
  'label',
  'description',
  'type',
  'target',
]);
const VARIABLE_TYPES = new Set(['text', 'password']);
const VARIABLE_TARGETS = new Set([
  'env',
  'header',
  'bearer-token',
  'url-param',
]);

const isPlainObject = (value) =>
  value !== null && typeof value === 'object' && !Array.isArray(value);

/**
 * Upstream data becomes a preset users install and run, so nothing is trusted:
 * the launcher must be `npx -y @fre4x/<pkg>[@version]` and every other field must
 * have the expected shape. Returns the reason an entry is rejected, or null.
 */
function validateUpstreamEntry(name, entry) {
  if (!NAME_RE.test(name)) return 'unexpected preset name';
  if (!isPlainObject(entry)) return 'entry is not an object';
  const unknown = Object.keys(entry).filter((key) => !ALLOWED_FIELDS.has(key));
  if (unknown.length) return `unexpected field(s): ${unknown.join(', ')}`;
  if (entry.command !== 'npx') return 'command must be "npx"';
  const args = entry.args;
  if (
    !Array.isArray(args) ||
    args.length !== 2 ||
    args[0] !== '-y' ||
    typeof args[1] !== 'string' ||
    !PACKAGE_ARG_RE.test(args[1])
  ) {
    return 'args must be exactly ["-y", "@fre4x/<pkg>[@version]"]';
  }
  for (const field of ['category', 'description', 'logo']) {
    if (entry[field] !== undefined && typeof entry[field] !== 'string') {
      return `${field} must be a string`;
    }
  }
  if (typeof entry.category !== 'string' || !entry.category) {
    return 'category is required';
  }
  if (entry.logo !== undefined && !/^https:\/\//.test(entry.logo)) {
    return 'logo must be an https URL';
  }
  if (entry.env !== undefined) {
    if (
      !isPlainObject(entry.env) ||
      !Object.entries(entry.env).every(
        ([key, value]) => ENV_KEY_RE.test(key) && typeof value === 'string',
      )
    ) {
      return 'env must map NAME to string values';
    }
    for (const key of Object.keys(entry.env)) {
      if (PROCESS_ENV_RE.test(key)) return `env ${key} affects the launcher`;
      if (
        !isPlainObject(entry.variableDefinitions) ||
        !(key in entry.variableDefinitions)
      ) {
        return `env ${key} is not declared in variableDefinitions`;
      }
    }
  }
  if (entry.variableDefinitions !== undefined) {
    const definitions = entry.variableDefinitions;
    if (!isPlainObject(definitions))
      return 'variableDefinitions must be an object';
    for (const [key, definition] of Object.entries(definitions)) {
      if (!ENV_KEY_RE.test(key) || !isPlainObject(definition)) {
        return 'variableDefinitions entries must be objects keyed by NAME';
      }
      if (PROCESS_ENV_RE.test(key)) {
        return `variableDefinitions ${key} affects the launcher`;
      }
      if (
        !Object.keys(definition).every((f) => ALLOWED_VARIABLE_FIELDS.has(f))
      ) {
        return `unexpected field in variableDefinitions.${key}`;
      }
      const { required, label, description, type, target } = definition;
      if (
        (required !== undefined && typeof required !== 'boolean') ||
        (label !== undefined && typeof label !== 'string') ||
        (description !== undefined && typeof description !== 'string') ||
        (type !== undefined && !VARIABLE_TYPES.has(type)) ||
        (target !== undefined && !VARIABLE_TARGETS.has(target))
      ) {
        return `invalid variableDefinitions.${key}`;
      }
    }
  }
  return null;
}

function fromUpstream(upstreamEntry, existing) {
  const entry = { ...upstreamEntry, args: upstreamEntry.args.map(toLatest) };
  if (entry.logo === undefined && existing?.logo !== undefined) {
    entry.logo = existing.logo;
  }
  return orderFields(entry);
}

/** JSON with object keys sorted, so equality ignores key order. */
function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, canonical(value[key])]),
    );
  }
  return value;
}

const same = (a, b) =>
  JSON.stringify(canonical(a)) === JSON.stringify(canonical(b));

/**
 * @returns {{ next: object, added: string[], changed: string[], removed: string[],
 *             skipped: string[], rejected: {name: string, reason: string}[],
 *             commandChanged: string[], previousFre4xCount: number, version: string | null }}
 */
function syncFre4xPresets(current, upstream) {
  const currentServers = current.mcpServers ?? {};
  // Every upstream entry that names an @fre4x package is a candidate; invalid ones
  // are rejected (and never remove an existing local entry of the same name).
  const candidates = Object.entries(upstream.mcpServers ?? {}).filter(
    ([, entry]) => isFre4x(entry),
  );
  const candidateNames = new Set(candidates.map(([name]) => name));
  const rejected = [];
  const valid = [];
  for (const [name, entry] of candidates) {
    const reason = validateUpstreamEntry(name, entry);
    if (reason) rejected.push({ name, reason });
    else valid.push([name, entry]);
  }
  const validNames = new Set(valid.map(([name]) => name));

  const added = [];
  const changed = [];
  const removed = [];
  const skipped = [];
  const commandChanged = [];
  const nextServers = {};
  const launch = (entry) => JSON.stringify([entry.command, entry.args]);

  for (const [name, entry] of Object.entries(currentServers)) {
    if (!isFre4x(entry)) {
      nextServers[name] = entry;
    } else if (!candidateNames.has(name)) {
      removed.push(name);
    } else if (!validNames.has(name)) {
      nextServers[name] = entry; // rejected upstream entry: keep what we have
    } else {
      const updated = fromUpstream(upstream.mcpServers[name], entry);
      if (!same(updated, entry)) changed.push(name);
      if (launch(updated) !== launch(entry)) commandChanged.push(name);
      nextServers[name] = updated;
    }
  }
  for (const [name, entry] of valid) {
    if (name in currentServers && !isFre4x(currentServers[name])) {
      skipped.push(name); // collides with a hand-maintained non-fre4x preset
      continue;
    }
    if (name in nextServers) continue;
    nextServers[name] = fromUpstream(entry, undefined);
    added.push(name);
  }

  return {
    next: { ...current, mcpServers: nextServers },
    added,
    changed,
    removed,
    skipped,
    rejected,
    commandChanged,
    previousFre4xCount: Object.values(currentServers).filter(isFre4x).length,
    version: typeof upstream.version === 'string' ? upstream.version : null,
  };
}

/** Upstream-controlled text in the PR body: no backticks, newlines or markup, bounded. */
function plain(value, max = 80) {
  const text = String(value)
    .replace(/\b[a-z][a-z0-9+.-]*:\/\//gi, '') // no autolinked URLs
    .replace(/[\u0000-\u001f\u007f`<>[\]()|*~]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
  return text.length > max ? `${text.slice(0, max - 1)}…` : text;
}

function summaryMarkdown(result) {
  const list = (title, names) =>
    names.length
      ? `- **${title}**: ${names.map((n) => `\`${plain(n)}\``).join(', ')}\n`
      : '';
  const launchLines = result.added
    .concat(result.changed)
    .filter((name, index, all) => all.indexOf(name) === index)
    .map((name) => {
      const entry = result.next.mcpServers[name];
      const flag = result.commandChanged.includes(name)
        ? ' **(launch command changed)**'
        : '';
      const env = Object.entries(entry.env ?? {})
        .map(([key, value]) => `${key}=${plain(value, 40)}`)
        .join(' ');
      return (
        `- \`${plain(name)}\`: \`${entry.command} ${entry.args.join(' ')}\`${flag}` +
        (env ? ` env: \`${env}\`` : '')
      );
    });
  // Many removals at once usually mean upstream changed shape, not that modules went away.
  const manyRemoved =
    result.removed.length >= 3 ||
    (result.removed.length > 0 &&
      result.removed.length * 2 > result.previousFre4xCount);
  return (
    `Syncs the \`@fre4x/*\` presets in \`mcp-server.json\` from b1te${
      result.version ? ` v${plain(result.version, 24)}` : ''
    } (\`presets.json\`).\n\n` +
    (manyRemoved
      ? `> **Review carefully:** ${result.removed.length} of ${result.previousFre4xCount} existing @fre4x presets are removed by this sync.\n\n`
      : '') +
    list('Added', result.added) +
    list('Updated', result.changed) +
    list('Removed', result.removed) +
    list('Skipped (name used by a non-fre4x preset)', result.skipped) +
    (result.rejected.length
      ? `- **Rejected (failed validation, left as is)**: ${result.rejected
          .map(({ name, reason }) => `\`${plain(name)}\` (${plain(reason)})`)
          .join(', ')}\n`
      : '') +
    (launchLines.length
      ? `\n**Launch commands and env to review**\n\n${launchLines.join('\n')}\n`
      : '') +
    '\nCommands use `@fre4x/<pkg>@latest`; non-fre4x presets are untouched. ' +
    'Generated by `scripts/sync-fre4x-presets.cjs`; never auto-merged.\n'
  );
}

function parseArgs(argv) {
  const args = { target: 'mcp-server.json', check: false };
  for (let i = 0; i < argv.length; i += 1) {
    const flag = argv[i];
    if (flag === '--check') args.check = true;
    else if (['--input', '--target', '--summary'].includes(flag)) {
      args[flag.slice(2)] = argv[(i += 1)];
    } else throw new Error(`Unknown argument: ${flag}`);
  }
  if (!args.input) throw new Error('--input <presets.json> is required');
  return args;
}

function main(argv) {
  const args = parseArgs(argv);
  const target = path.resolve(args.target);
  const current = JSON.parse(fs.readFileSync(target, 'utf8'));
  const upstream = JSON.parse(
    fs.readFileSync(path.resolve(args.input), 'utf8'),
  );
  const result = syncFre4xPresets(current, upstream);
  const changed = !same(result.next, current);

  if (changed && !args.check) {
    fs.writeFileSync(target, `${JSON.stringify(result.next, null, 2)}\n`);
  }
  if (changed && args.summary) {
    fs.writeFileSync(path.resolve(args.summary), summaryMarkdown(result));
  }
  for (const name of result.skipped) {
    console.warn(`skipped ${name}: name is used by a non-fre4x preset`);
  }
  for (const { name, reason } of result.rejected) {
    console.warn(`rejected ${name}: ${reason}`);
  }
  console.log(`changed=${changed}`);
  return changed;
}

if (require.main === module) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    console.error(error.message);
    process.exit(2);
  }
}

module.exports = { isFre4x, toLatest, syncFre4xPresets, summaryMarkdown };
