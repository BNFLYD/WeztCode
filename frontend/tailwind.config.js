/** @type {import('tailwindcss').Config} */
export default {
  content: [
    './src/**/*.{html,js,svelte,ts}',
    './src/**/**/*.{html,js,svelte,ts}'
  ],
  theme: {
    extend: {
      colors: {
        // Main accent color (cyan)
        accent: '#ac57ff',
        // Background colors
        back: '#1e1e1e',
        'back-deep': '#0d0d0d',
        // Text color
        text: '#d0d0d0',
      },
    },
  },
  plugins: [
    require('tailwindcss-animate')
  ],
}
