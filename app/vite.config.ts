import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

/**
 * The app build. `npm run build` at the workspace root builds the module
 * first, then this: a self-contained static bundle in `app/dist` that fetches
 * nothing but its own files.
 */
export default defineConfig({
  plugins: [react()],
  // The worker is a module worker, so its chunk is one too.
  worker: { format: 'es' },
  server: {
    // The worker package and the authored content sit beside the app.
    fs: { allow: ['..'] },
  },
  build: {
    target: 'es2022',
    emptyOutDir: true,
  },
});
