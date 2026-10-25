import { fileURLToPath, URL } from 'node:url';
import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// Development: the API is the admin listener of the native laboratory frontend
// (scripts/dev/start-lab-frontend.ps1 -AdminPort 27090); production files are served by it.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
  server: {
    host: '127.0.0.1',
    port: 5173,
    strictPort: true,
    proxy: { '/api': { target: 'http://127.0.0.1:27090', changeOrigin: true } },
  },
  build: { outDir: 'dist', sourcemap: false },
});
