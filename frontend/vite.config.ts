/// <reference types="vitest/config" />
import path from 'node:path';
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    port: 8100,
    strictPort: true,
  },
  // Read env (VITE_*) from the repo root so a single .env serves all workspaces.
  // Only VITE_-prefixed vars are exposed to the client (explicit for review).
  envDir: path.resolve(import.meta.dirname, '..'),
  envPrefix: 'VITE_',
  resolve: {
    alias: {
      '@': path.resolve(import.meta.dirname, './src'),
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
    globals: false,
    css: false,
  },
});
