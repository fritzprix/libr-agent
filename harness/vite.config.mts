import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

const here = fileURLToPath(new URL('.', import.meta.url));
const repo = fileURLToPath(new URL('../../..', import.meta.url));

export default defineConfig({
  root: here,
  base: './',
  plugins: [react()],
  resolve: {
    alias: [
      { find: '@/lib/backend/core', replacement: `${here}mockCore.ts` },
      { find: '@/lib/backend/assistants', replacement: `${here}mockAssistants.ts` },
      { find: '@/lib/backend', replacement: `${here}mockBackend.ts` },
      { find: '@/lib/logger', replacement: `${here}mockLogger.ts` },
      { find: '@', replacement: `${repo}src` },
    ],
  },
  server: { port: 5199, strictPort: true },
});
