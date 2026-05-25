/// <reference types="vitest" />
import { defineConfig, mergeConfig, configDefaults } from 'vitest/config';
import viteConfig from './vite.config';

// Call the function to get the config object
const viteConfigObject = viteConfig({ command: 'serve', mode: 'test' });
const lowMemoryValidate = process.env.LIBRAGENT_LOW_MEMORY_VALIDATE === '1';

export default mergeConfig(
  viteConfigObject,
  defineConfig({
    test: {
      globals: true,
      environment: 'jsdom',
      setupFiles: './src/test/setup.ts',
      css: true,
      exclude: [...configDefaults.exclude, 'aur/**', '.worktrees/**'],
      fileParallelism: !lowMemoryValidate,
      maxWorkers: lowMemoryValidate ? 1 : undefined,
      minWorkers: lowMemoryValidate ? 1 : undefined,
    },
  }),
);
