import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

const scriptPath = path.join(process.cwd(), 'scripts/enforce-pnpm-version.cjs');

describe('enforce-pnpm-version', () => {
  it('skips enforcement for non-pnpm package managers', () => {
    expect(() =>
      execFileSync(process.execPath, [scriptPath], {
        env: {
          ...process.env,
          npm_config_user_agent: 'npm/10.0.0 node/v20.0.0',
        },
        stdio: ['ignore', 'pipe', 'pipe'],
      }),
    ).not.toThrow();
  });
});
