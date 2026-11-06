import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Cargo holds locks on files under src-tauri/target, which makes
      // chokidar throw EBUSY and kill the dev server while Tauri awaits it.
      ignored: ['**/src-tauri/**'],
    },
  },
})
