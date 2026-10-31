import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { tauri } from '@tauri-apps/cli/vite'

export default defineConfig({
  plugins: [react(), tauri()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
})