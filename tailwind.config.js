/** @type {import('tailwindcss').Config} */
export default {
  // Use class-based dark mode so ThemeProvider (next-themes with attribute="class")
  // can toggle the `.dark` class on the document root and Tailwind's `dark:`
  // variants will respond to it. The project defines `.dark { ... }` CSS
  // variables in `src/styles/globals.css` and relies on class-based switching.
  darkMode: 'class',
  content: [
    './index.html',
    './src/**/*.{js,ts,jsx,tsx}',
    './src/app/**/*.{js,ts,jsx,tsx}',
    './src/components/**/*.{js,ts,jsx,tsx}',
    './src/features/**/*.{js,ts,jsx,tsx}',
  ],
  theme: {
    extend: {},
  },
  plugins: [],
};
