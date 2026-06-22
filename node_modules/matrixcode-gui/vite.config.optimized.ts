import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],

  // Performance optimization: Code splitting
  build: {
    rollupOptions: {
      output: {
        // Manual chunks for better caching and lazy loading
        manualChunks: {
          // Vendor libraries - rarely change
          'vendor-react': ['react', 'react-dom'],
          'vendor-tauri': ['@tauri-apps/api/core', '@tauri-apps/api/event'],
          'vendor-markdown': ['react-markdown', 'react-syntax-highlighter'],

          // UI libraries
          'vendor-ui': ['zustand'],

          // Heavy dependencies
          'vendor-syntax': [
            'react-syntax-highlighter/dist/esm/styles/prism',
            'react-syntax-highlighter/dist/esm/languages/prism/javascript',
            'react-syntax-highlighter/dist/esm/languages/prism/typescript',
            'react-syntax-highlighter/dist/esm/languages/prism/python',
            'react-syntax-highlighter/dist/esm/languages/prism/bash',
          ],
        },
      },
    },

    // Chunk size warnings
    chunkSizeWarningLimit: 600, // Increase limit for main chunk

    // Target modern browsers for better performance
    target: 'esnext',

    // Minification options
    minify: 'esbuild',
    esbuild: {
      // Remove console logs in production (optional)
      // drop: ['console', 'debugger'],
    },
  },

  // Optimize dependency pre-bundling
  optimizeDeps: {
    include: [
      'react',
      'react-dom',
      '@tauri-apps/api/core',
      '@tauri-apps/api/event',
      'zustand',
      'react-markdown',
    ],
    exclude: [
      // Large dependencies that should be lazy loaded
      'react-syntax-highlighter',
    ],
  },

  // Development optimizations
  server: {
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