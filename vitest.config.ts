/// <reference types="vitest" />
import { defineConfig, mergeConfig, configDefaults } from 'vitest/config';
import viteConfig from './vite.config';

// Call the function to get the config object
const viteConfigObject = viteConfig({ command: 'serve', mode: 'test' });

export default mergeConfig(
  viteConfigObject,
  defineConfig({
    test: {
      globals: true,
      environment: 'jsdom',
      setupFiles: './src/test/setup.ts',
      css: true,
      exclude: [...configDefaults.exclude, 'aur/**', '.worktrees/**'],
    },
  }),
);
