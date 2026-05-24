/// <reference types="vitest" />
import { defineConfig, mergeConfig, configDefaults } from 'vitest/config';
import viteConfig from './vite.config';

// Call the function to get the config object
const viteConfigObject = viteConfig({ command: 'serve', mode: 'test' });

function parsePositiveIntEnv(name: string): number | undefined {
  const rawValue = process.env[name];
  if (!rawValue) {
    return undefined;
  }

  const parsed = Number.parseInt(rawValue, 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : undefined;
}

function parseBooleanEnv(name: string): boolean | undefined {
  const rawValue = process.env[name];
  if (!rawValue) {
    return undefined;
  }

  if (rawValue === 'true') {
    return true;
  }

  if (rawValue === 'false') {
    return false;
  }

  return undefined;
}

const maxWorkers = parsePositiveIntEnv('VITEST_MAX_WORKERS');
const minWorkers = parsePositiveIntEnv('VITEST_MIN_WORKERS');
const fileParallelism = parseBooleanEnv('VITEST_FILE_PARALLELISM');

export default mergeConfig(
  viteConfigObject,
  defineConfig({
    test: {
      globals: true,
      environment: 'jsdom',
      setupFiles: './src/test/setup.ts',
      css: true,
      exclude: [...configDefaults.exclude, 'aur/**', '.worktrees/**'],
      ...(maxWorkers ? { maxWorkers } : {}),
      ...(minWorkers ? { minWorkers } : {}),
      ...(fileParallelism !== undefined ? { fileParallelism } : {}),
    },
  }),
);
