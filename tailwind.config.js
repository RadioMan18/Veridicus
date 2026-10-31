/** @type {import('tailwindcss').Config} */
export default {
  content: [
    './index.html',
    './src/**/*.{js,ts,jsx,tsx}',
  ],
  theme: {
    extend: {
      colors: {
        aether: {
          bg: 'var(--aether-bg)',
          surface: 'var(--aether-surface)',
          'surface-border': 'var(--aether-border)',
          text: 'var(--aether-text)',
          'text-muted': 'var(--aether-text-muted)',
          primary: 'var(--aether-primary)',
          'primary-hover': 'var(--aether-primary-hover)',
          'primary-fg': 'var(--aether-primary-fg)',
          selected: 'var(--aether-selected)',
          'selected-border': 'var(--aether-selected-border)',
          'selected-fg': 'var(--aether-selected-fg)',
          success: 'var(--aether-success)',
          warning: 'var(--aether-warning)',
          error: 'var(--aether-error)',
          info: 'var(--aether-info)',
        },
      },
    },
  },
  plugins: [],
}
