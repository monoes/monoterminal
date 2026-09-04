import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { VitePWA } from 'vite-plugin-pwa'

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    react(),
    VitePWA({
      registerType: 'autoUpdate',
      includeAssets: ['favicon.ico', 'robots.txt', 'apple-touch-icon.png'],
      manifest: {
        name: 'MONOTERMINAL',
        short_name: 'MonoTerm',
        description: 'Next-generation terminal emulator with P2P collaboration',
        theme_color: '#1e1e1e',
        background_color: '#1e1e1e',
        display: 'standalone',
        display_override: ['window-controls-overlay'],
        orientation: 'any',
        scope: '/',
        start_url: '/',
        icons: [
          {
            src: 'pwa-192x192.png',
            sizes: '192x192',
            type: 'image/png',
          },
          {
            src: 'pwa-512x512.png',
            sizes: '512x512',
            type: 'image/png',
          },
          {
            src: 'pwa-512x512.png',
            sizes: '512x512',
            type: 'image/png',
            purpose: 'any maskable',
          },
        ],
        file_handlers: [
          {
            action: '/',
            accept: {
              'text/plain': ['.txt', '.log'],
            },
          },
        ],
      },
      workbox: {
        globPatterns: ['**/*.{js,css,html,ico,png,svg,woff2}'],
        runtimeCaching: [
          {
            urlPattern: /^https:\/\/fonts\.googleapis\.com\/.*/i,
            handler: 'CacheFirst',
            options: {
              cacheName: 'google-fonts-cache',
              expiration: {
                maxEntries: 10,
                maxAgeSeconds: 60 * 60 * 24 * 365, // 1 year
              },
              cacheableResponse: {
                statuses: [0, 200],
              },
            },
          },
        ],
      },
    }),
  ],
  server: {
    port: 3000,
    // Explicit IPv4 loopback: Node's default DNS resolution for the
    // 'localhost' hostname on this machine returns ::1 first, so binding
    // without an explicit host only listened on ::1 — any client (browser
    // or otherwise) whose resolver or happy-eyeballs picked 127.0.0.1 first
    // got a hard connection-refused instead of falling through.
    host: '127.0.0.1',
    proxy: {
      '/ws': {
        target: 'ws://localhost:54321',
        ws: true,
      },
    },
  },
  preview: {
    port: 8080,
    strictPort: true,
  },
})
