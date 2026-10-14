import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';
import { offline } from './scripts/offline.mjs';

export default defineConfig({
  // Relative paths, so that the web app works from wherever it is put.
  base: './',
  plugins: [svelte(), offline()],
  // Tauri expects a fixed port and prints its own output to the same terminal.
  clearScreen: false,
  server: { port: 5173, strictPort: true },
});
