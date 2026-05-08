/** @type {import('tailwindcss').Config} */
export default {
  content: [
    './src/**/*.{html,js,svelte,ts}',
    './src/**/**/*.{html,js,svelte,ts}'
  ],
  theme: {
    extend: {
      fontFamily: {
        'sans': ['"Rajdhani"', '"M PLUS 2"'],
        'mono': ['"JetBrains Mono"', '"M PLUS 1 Code"'],
        'code': ['"Share Tech Mono"', '"M PLUS 1 Code"'],
        'arcade': ['"Silkscreen"', '"DotGothic16"'],
        'specs': ['"Share Tech Mono"', '"Workbench"'],
        'ascii': ['monospace'],
        'mark': ['"Dela Gothic One"', '"Wavefont"'],
      },
      colors: {
        // Accent colors
        accent: {
          DEFAULT: '#00ffdd',
          contrast: '#efede3',
          detail: '#c5bfae',
          warn: '#ac57ff',
          err: '#ff3370',
        },
        // Background colors
        back: {
          DEFAULT: '#302f2c',
          deep: '#0d0d0d',
        },
        // Text colors
        print: {
          DEFAULT: '#d0d0d0',
          contrast: '#efede3',
          tag: '#c5bfae',
        },
      },
    },
  },
  plugins: [
    require('tailwindcss-animate')
  ],
}
