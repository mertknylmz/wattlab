import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

// The build is the desktop app's renderer: Electron serves these files from its
// own wattlab:// scheme. There is no browser/web deployment.
export default defineConfig({
  plugins: [react()],
  base: './',
  build: {
    outDir: '../wattlab-desktop/renderer',
    emptyOutDir: true,
    assetsDir: '',
    cssCodeSplit: false,
    modulePreload: { polyfill: false },
    rollupOptions: {
      output: {
        entryFileNames: 'app.js',
        inlineDynamicImports: true,
        assetFileNames: (asset) => (asset.name?.endsWith('.css') ? 'styles.css' : '[name][extname]'),
      },
    },
  },
  test: {
    environment: 'jsdom',
    pool: 'threads',
  },
});
