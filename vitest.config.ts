import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

/**
 * The worker and app tests. `npm test` runs them between the core tests and
 * the workspace checks.
 *
 * Files run one at a time: the production-build test rewrites the generated
 * module the worker test loads, so they must not overlap.
 */
export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    include: ['worker/test/**/*.test.ts', 'app/test/**/*.test.{ts,tsx}'],
    globalSetup: ['./vitest.global-setup.ts'],
    fileParallelism: false,
    testTimeout: 30_000,
  },
});
