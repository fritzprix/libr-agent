import { createRequire } from 'node:module';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';

const require = createRequire(import.meta.url);
const {
  normalizeHermesJob,
  normalizeHermesVersion,
} = require('./normalize-harbor-agent-metadata.cjs');

describe('normalizeHermesVersion', () => {
  it('normalizes Hermes version to its release line', () => {
    expect(
      normalizeHermesVersion(
        'Hermes Agent v0.19.0 (2026.7.20)\n' +
          'Install directory: /root/.local/share/uv/tools/hermes-agent\n' +
          'Python: 3.14.7',
      ),
    ).toBe('Hermes Agent v0.19.0 (2026.7.20)');
    expect(normalizeHermesVersion('LibrAgent 0.9.6')).toBeNull();
    expect(normalizeHermesVersion(null)).toBeNull();
  });
});

describe('normalizeHermesJob', () => {
  const temporaryJobDirs = [];

  afterEach(() => {
    for (const jobDir of temporaryJobDirs.splice(0)) {
      fs.rmSync(jobDir, { recursive: true, force: true });
    }
  });

  it('keeps result and trajectory agent metadata consistent', () => {
    const jobDir = fs.mkdtempSync(
      path.join(os.tmpdir(), 'libragent-harbor-normalize-'),
    );
    temporaryJobDirs.push(jobDir);
    const trialDir = path.join(jobDir, 'task__abc123');
    const agentDir = path.join(trialDir, 'agent');
    fs.mkdirSync(agentDir, { recursive: true });

    const version =
      'Hermes Agent v0.19.0 (2026.7.20)\n' +
      'Install directory: /root/.local/share/uv/tools/hermes-agent/lib/python3.14\n' +
      'Python: 3.14.7';
    fs.writeFileSync(
      path.join(trialDir, 'result.json'),
      JSON.stringify({ agent_info: { name: 'hermes', version } }),
    );
    fs.writeFileSync(
      path.join(agentDir, 'trajectory.json'),
      JSON.stringify({ agent: { name: 'hermes', version } }),
    );

    expect(normalizeHermesJob(jobDir)).toBe(2);
    expect(
      JSON.parse(fs.readFileSync(path.join(trialDir, 'result.json'), 'utf8'))
        .agent_info.version,
    ).toBe('Hermes Agent v0.19.0 (2026.7.20)');
    expect(
      JSON.parse(
        fs.readFileSync(path.join(agentDir, 'trajectory.json'), 'utf8'),
      ).agent.version,
    ).toBe('Hermes Agent v0.19.0 (2026.7.20)');
  });
});
