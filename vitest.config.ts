import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    // Only the pure modules are unit tested; the UI is exercised through Tauri.
    include: ['src/**/*.test.ts'],
    environment: 'node',
  },
})
