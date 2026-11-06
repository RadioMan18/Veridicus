/** @type {import('tailwindcss').Config} */
export default {
  content: [
    './index.html',
    './src/**/*.{js,ts,jsx,tsx}',
  ],
  theme: {
    extend: {
      // Five steps, line heights on an 8px grid (16/24/32/40).
      fontSize: {
        display: ['1.75rem', { lineHeight: '2.5rem', letterSpacing: '-0.02em', fontWeight: '700' }],
        section: ['1.25rem', { lineHeight: '2rem', letterSpacing: '-0.01em', fontWeight: '600' }],
        row: ['0.9375rem', { lineHeight: '1.5rem' }],
        body: ['0.875rem', { lineHeight: '1.5rem' }],
        meta: ['0.75rem', { lineHeight: '1rem' }],
      },
      colors: {
        aether: {
          bg: 'var(--aether-bg)',
          surface: 'var(--aether-surface)',
          border: 'var(--aether-border)',
          'surface-border': 'var(--aether-border)',
          text: 'var(--aether-text)',
          'text-muted': 'var(--aether-text-muted)',
          primary: 'var(--aether-primary)',
          'primary-hover': 'var(--aether-primary-hover)',
          'primary-fg': 'var(--aether-primary-fg)',
          'primary-ink': 'var(--aether-primary-ink)',
          selected: 'var(--aether-selected)',
          'selected-border': 'var(--aether-selected-border)',
          'selected-fg': 'var(--aether-selected-fg)',
          success: 'var(--aether-success)',
          warning: 'var(--aether-warning)',
          'warning-hover': 'var(--aether-warning-hover, var(--aether-warning))',
          error: 'var(--aether-error)',
          info: 'var(--aether-info)',
        },
      },
    },
  },
  plugins: [],
}
