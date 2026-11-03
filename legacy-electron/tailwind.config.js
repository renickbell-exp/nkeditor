/** @type {import('tailwindcss').Config} */
export default {
  content: [
    './index.html',
    './src/renderer/**/*.{js,ts,jsx,tsx}',
  ],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        // nanoKONTROL2 inspired colors - use CSS variables for runtime theming
        'nk-dark': 'var(--nk-dark, #1a1a1a)',
        'nk-darker': 'var(--nk-darker, #0f0f0f)',
        'nk-light': 'var(--nk-light, #2a2a2a)',
        'nk-border': 'var(--nk-border, #3a3a3a)',
        'nk-accent': 'var(--nk-accent, #ff6600)',
        'nk-solo': 'var(--nk-solo, #ffcc00)',
        'nk-mute': 'var(--nk-mute, #00cc66)',
        'nk-rec': 'var(--nk-rec, #ff3333)',
      },
      fontFamily: {
        sans: ['var(--nk-font-family, Inter)', 'Roboto', 'Arial', 'system-ui', '-apple-system', 'BlinkMacSystemFont', 'Segoe UI', 'sans-serif'],
        mono: ['JetBrains Mono', 'SF Mono', 'Menlo', 'Monaco', 'Cascadia Code', 'monospace'],
      },
    },
  },
  plugins: [],
};
