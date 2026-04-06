/// <reference types="vitest" />
import { defineConfig } from 'vite';

export default defineConfig({
  test: {
    globals: true,
    testTimeout: 60_000,
    include: ['tests/**/*.test.ts'],
  },
});
