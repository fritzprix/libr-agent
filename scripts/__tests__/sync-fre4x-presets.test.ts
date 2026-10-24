import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';
import { describe, expect, it } from 'vitest';

const require = createRequire(import.meta.url);
const scriptPath = path.join(process.cwd(), 'scripts/sync-fre4x-presets.cjs');
const { syncFre4xPresets, toLatest } = require(scriptPath) as {
  syncFre4xPresets: (
    current: Config,
    upstream: Config & { version?: string },
  ) => {
    next: Config;
    added: string[];
    changed: string[];
    removed: string[];
    skipped: string[];
  };
  toLatest: (arg: string) => string;
};

type Entry = Record<string, unknown>;
type Config = { mcpServers: Record<string, Entry> };

const fre4x = (pkg: string, extra: Entry = {}, version = '1.1.29'): Entry => ({
  category: 'search',
  description: `${pkg} server`,
  command: 'npx',
  args: ['-y', `@fre4x/${pkg}@${version}`],
  ...extra,
});

const handMade: Config['mcpServers'] = {
  'ddg-search': {
    category: 'search',
    command: 'uvx',
    args: ['duckduckgo-mcp-server'],
  },
  github: { category: 'devtools', url: 'https://example.test/mcp' },
};

const local = (): Config => ({
  mcpServers: {
    hn: fre4x('hn', { logo: 'https://example.test/hn.ico' }, 'latest'),
    'ddg-search': handMade['ddg-search'],
    github: handMade.github,
  },
});

describe('sync-fre4x-presets', () => {
  it('records pinned versions as @latest', () => {
    expect(toLatest('@fre4x/arxiv@1.1.29')).toBe('@fre4x/arxiv@latest');
    expect(toLatest('@fre4x/arxiv')).toBe('@fre4x/arxiv@latest');
    expect(toLatest('-y')).toBe('-y');
    expect(toLatest('duckduckgo-mcp-server')).toBe('duckduckgo-mcp-server');
  });

  it('adds a new module with no manual edits', () => {
    const upstream = {
      mcpServers: { hn: fre4x('hn'), benchmark: fre4x('benchmark') },
    };
    const result = syncFre4xPresets(local(), upstream);
    expect(result.added).toEqual(['benchmark']);
    expect(result.next.mcpServers.benchmark.args).toEqual([
      '-y',
      '@fre4x/benchmark@latest',
    ]);
  });

  it('is idempotent: a version-only release changes nothing', () => {
    const current = local();
    const upstream = {
      version: '1.1.30',
      mcpServers: { hn: fre4x('hn', {}, '1.1.30') },
    };
    const first = syncFre4xPresets(current, upstream);
    expect(first.next).toEqual(current);
    expect(first.added.concat(first.changed, first.removed)).toEqual([]);

    const withNew = syncFre4xPresets(current, {
      mcpServers: { ...upstream.mcpServers, fred: fre4x('fred') },
    });
    const again = syncFre4xPresets(withNew.next, {
      mcpServers: { ...upstream.mcpServers, fred: fre4x('fred', {}, '1.1.31') },
    });
    expect(again.next).toEqual(withNew.next);
  });

  it('never modifies hand-maintained non-fre4x presets', () => {
    const upstream = {
      mcpServers: {
        hn: fre4x('hn', { description: 'changed upstream' }),
        github: fre4x('github'),
        'ddg-search': fre4x('ddg-search'),
      },
    };
    const result = syncFre4xPresets(local(), upstream);
    expect(result.skipped.sort()).toEqual(['ddg-search', 'github']);
    expect(result.next.mcpServers['ddg-search']).toEqual(
      handMade['ddg-search'],
    );
    expect(result.next.mcpServers.github).toEqual(handMade.github);
    expect(result.changed).toEqual(['hn']);
  });

  it('removes fre4x presets b1te dropped but keeps everything else', () => {
    const current = local();
    current.mcpServers.gone = fre4x('gone', {}, 'latest');
    const result = syncFre4xPresets(current, {
      mcpServers: { hn: fre4x('hn') },
    });
    expect(result.removed).toEqual(['gone']);
    expect(Object.keys(result.next.mcpServers)).toEqual([
      'hn',
      'ddg-search',
      'github',
    ]);
  });

  it('keeps a curated logo when upstream ships none, and prefers an upstream one', () => {
    const kept = syncFre4xPresets(local(), { mcpServers: { hn: fre4x('hn') } });
    expect(kept.next.mcpServers.hn.logo).toBe('https://example.test/hn.ico');
    const replaced = syncFre4xPresets(local(), {
      mcpServers: { hn: fre4x('hn', { logo: 'https://example.test/new.ico' }) },
    });
    expect(replaced.next.mcpServers.hn.logo).toBe(
      'https://example.test/new.ico',
    );
  });

  it('the CLI rewrites the target only when something changed', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'fre4x-sync-'));
    const target = path.join(dir, 'mcp-server.json');
    const input = path.join(dir, 'presets.json');
    const summary = path.join(dir, 'summary.md');
    fs.writeFileSync(target, JSON.stringify(local()));
    const run = () =>
      execFileSync(
        process.execPath,
        [
          scriptPath,
          '--input',
          input,
          '--target',
          target,
          '--summary',
          summary,
        ],
        { encoding: 'utf8' },
      );

    fs.writeFileSync(
      input,
      JSON.stringify({ mcpServers: { hn: fre4x('hn', {}, '9.9.9') } }),
    );
    const before = fs.readFileSync(target, 'utf8');
    expect(run()).toContain('changed=false');
    expect(fs.readFileSync(target, 'utf8')).toBe(before);
    expect(fs.existsSync(summary)).toBe(false);

    fs.writeFileSync(
      input,
      JSON.stringify({
        version: '1.2.0',
        mcpServers: { hn: fre4x('hn'), fred: fre4x('fred') },
      }),
    );
    expect(run()).toContain('changed=true');
    const written = JSON.parse(fs.readFileSync(target, 'utf8')) as Config;
    expect(Object.keys(written.mcpServers)).toContain('fred');
    expect(fs.readFileSync(summary, 'utf8')).toContain('`fred`');
    expect(run()).toContain('changed=false');
  });

  it('the repository mcp-server.json is stable under its own fre4x entries', () => {
    const repoConfig = JSON.parse(
      fs.readFileSync(path.join(process.cwd(), 'mcp-server.json'), 'utf8'),
    ) as Config;
    const upstream: Config = {
      mcpServers: Object.fromEntries(
        Object.entries(repoConfig.mcpServers)
          .filter(([, entry]) => JSON.stringify(entry).includes('@fre4x/'))
          .map(([name, entry]) => [
            name,
            {
              ...entry,
              args: (entry.args as string[]).map((a) =>
                a.replace('@latest', '@1.2.3'),
              ),
            },
          ]),
      ),
    };
    const result = syncFre4xPresets(repoConfig, upstream);
    expect(result.next).toEqual(repoConfig);
  });
});
