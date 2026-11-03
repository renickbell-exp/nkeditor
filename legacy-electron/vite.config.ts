import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import electron from 'vite-plugin-electron';
import renderer from 'vite-plugin-electron-renderer';
import { resolve } from 'path';
import { builtinModules } from 'module';

export default defineConfig({
  plugins: [
    react(),
    electron([
      {
        entry: 'src/main/main.ts',
        onstart(options) {
          options.startup();
        },
        vite: {
          build: {
            outDir: 'dist-electron',
            minify: false,
            rollupOptions: {
              external: ['easymidi'],
              output: {
                format: 'cjs',
                entryFileNames: '[name].cjs',
              },
            },
          },
          resolve: {
            alias: {
              '@': resolve(__dirname, 'src'),
              '@shared': resolve(__dirname, 'src/shared'),
              '@main': resolve(__dirname, 'src/main'),
            },
          },
        },
      },
      {
        entry: 'src/main/preload.ts',
        onstart(options) {
          options.reload();
        },
        vite: {
          build: {
            outDir: 'dist-electron',
            minify: false,
            rollupOptions: {
              output: {
                format: 'cjs',
                entryFileNames: '[name].cjs',
              },
            },
          },
          resolve: {
            alias: {
              '@': resolve(__dirname, 'src'),
              '@shared': resolve(__dirname, 'src/shared'),
              '@main': resolve(__dirname, 'src/main'),
            },
          },
        },
      },
    ]),
    renderer(),
  ],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
      '@shared': resolve(__dirname, 'src/shared'),
      '@renderer': resolve(__dirname, 'src/renderer'),
      '@main': resolve(__dirname, 'src/main'),
    },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
  // Vitest configuration: bypass the vite-plugin-electron-renderer shims
  // so that Node.js built-in modules (fs, path, etc.) resolve to the real
  // implementations rather than the ESM-incompatible .vite-electron-renderer shims.
  test: {
    alias: Object.fromEntries(
      builtinModules.flatMap((mod) => [
        [mod, mod],
        [`node:${mod}`, `node:${mod}`],
      ]),
    ),
  },
});
