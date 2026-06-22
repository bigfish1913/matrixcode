/// <reference types="vitest" />
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],

  // Fix ESM compatibility for react-syntax-highlighter
  resolve: {
    alias: {
      // Redirect deprecated import path to correct ESM path
      '@babel/runtime/regenerator': path.resolve(
        __dirname,
        'node_modules/@babel/runtime/helpers/regeneratorRuntime.js'
      ),
    },
  },

  // Vitest configuration
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./tests/setup.ts'],
    include: ['tests/**/*.test.{ts,tsx}'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
      exclude: [
        'node_modules/',
        'tests/',
        '**/*.d.ts',
        '**/*.config.*',
        '**/dist/**',
      ],
    },
  },

  // Performance optimization: Enhanced code splitting (Phase 6 - TUI/VSCode alignment optimization)
  build: {
    rollupOptions: {
      output: {
        // Enhanced manual chunks for better caching and performance
        manualChunks: (id) => {
          // React core - most stable, rarely changes
          if (id.includes('react') && id.includes('react-dom')) {
            return 'vendor-react';
          }

          // Tauri API - stable backend integration
          if (id.includes('@tauri-apps/api')) {
            return 'vendor-tauri';
          }

          // Zustand - lightweight state management
          if (id.includes('zustand')) {
            return 'vendor-ui';
          }

          // React Markdown - split from syntax highlighter to reduce bundle size
          if (id.includes('react-markdown') && !id.includes('react-syntax-highlighter')) {
            return 'vendor-markdown-core';
          }

          // Syntax Highlighter - heavy dependency, further split
          if (id.includes('react-syntax-highlighter')) {
            // Styles - can be loaded separately
            if (id.includes('/styles/prism')) {
              return 'vendor-syntax-styles';
            }
            // Languages - can be lazy loaded
            if (id.includes('/languages/prism/')) {
              return 'vendor-syntax-langs';
            }
            // Core
            return 'vendor-syntax-core';
          }

          // Large UI components - lazy loaded via dynamic imports in App.tsx
          if (id.includes('SettingsPanel') || id.includes('TaskView') || id.includes('LspStatusPanel')) {
            return 'components-lazy';
          }
        },
      },
    },

    // Increase chunk size limit for markdown bundle (expected large)
    chunkSizeWarningLimit: 800,

    // Target modern browsers for better performance
    target: 'esnext',

    // Minification options
    minify: 'esbuild',
    esbuild: {
      // Remove console logs in production for better performance
      drop: ['console', 'debugger'],
    },
  },

  // Enhanced dependency pre-bundling optimization
  optimizeDeps: {
    include: [
      'react',
      'react-dom',
      '@tauri-apps/api/core',
      '@tauri-apps/api/event',
      'zustand',
      'react-markdown',
      // Force pre-bundling of CommonJS dependencies used by react-syntax-highlighter
      'lowlight',
      'highlight.js',
    ],
    exclude: [
      // Large dependencies that should be lazy loaded
      'react-syntax-highlighter',
    ],
  },

  // Development optimizations
  server: {
    // Fixed port for Tauri dev
    port: 1420,
    strictPort: true,
    // Faster HMR
    hmr: {
      overlay: true,
    },
  },

  // CSS optimization
  css: {
    // Minify CSS
    devSourcemap: false,
  },
});