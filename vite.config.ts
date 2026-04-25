import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'
import { readFileSync } from 'fs'

const pkg = JSON.parse(readFileSync(path.resolve(__dirname, 'package.json'), 'utf8')) as {
  version: string
  name: string
}

export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
  ],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    port: 9527,
  },
  build: {
    outDir: 'dist',
    chunkSizeWarningLimit: 1100,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes('node_modules')) {
            if (id.includes('/@codemirror/') || id.includes('/codemirror/')) return 'vendor-editor'
            if (id.includes('/lucide-react/')) return 'vendor-icons'
            if (id.includes('/@tauri-apps/')) return 'vendor-tauri'
            return 'vendor-core'
          }
          if (id.includes('/src/modules/settings/')) return 'settings'
          if (id.includes('/src/modules/git/')) return 'git-workbench'
          if (id.includes('/src/components/memory/')) return 'memory'
          return undefined
        },
      },
    },
  },
  // 编译期常量：与 package.json / tauri.conf.json / Cargo.toml 通过
  // scripts/release-macos.sh 同步 bump，单一信源 = package.json。
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
    __APP_NAME__: JSON.stringify(pkg.name),
  },
})
