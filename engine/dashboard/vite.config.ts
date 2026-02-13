import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';
import { visualizer } from 'rollup-plugin-visualizer';

const BACKEND_TARGET = process.env.FLAPJACK_BACKEND_URL || 'http://localhost:7700';

export default defineConfig({
  plugins: [
    react(),
    visualizer({
      filename: 'dist/stats.html',
      open: false,
      gzipSize: true,
      brotliSize: true,
    }),
  ],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  define: {
    __BACKEND_URL__: JSON.stringify(BACKEND_TARGET),
  },
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
    rollupOptions: {
      output: {
        manualChunks: {
          'react-vendor': ['react', 'react-dom', 'react-router-dom'],
          'query-vendor': ['@tanstack/react-query', 'axios'],
          'ui-vendor': [
            '@radix-ui/react-accordion',
            '@radix-ui/react-dialog',
            '@radix-ui/react-dropdown-menu',
            '@radix-ui/react-select',
            '@radix-ui/react-tabs',
            '@radix-ui/react-toast',
          ],
          'monaco': ['@monaco-editor/react'],
        },
      },
    },
    target: 'es2020',
    minify: 'esbuild',
  },
  server: {
    port: 5177,
    proxy: {
      '/1': BACKEND_TARGET,
      '/2': BACKEND_TARGET,
      '/health': BACKEND_TARGET,
      '/internal': BACKEND_TARGET,
      '/api-docs': BACKEND_TARGET,
      '/swagger-ui': BACKEND_TARGET,
    },
  },
});
