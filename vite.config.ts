import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// Конфигурация синхронизирована с src-tauri/tauri.conf.json:
// devUrl = http://localhost:1420, frontendDist = ../dist.
// https://tauri.app/develop/#prevent-conflicts-with-frontend-tooling
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Не пересобирать фронтенд из-за изменений в Rust-коде.
      ignored: ["**/src-tauri/**", "**/crates/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    // Проект целится только в Windows (WebView2 = Chromium) — см. план,
    // раздел "Инсталлятор под Windows". Единый таргет вместо ветвления
    // по TAURI_ENV_PLATFORM, которого нет при обычном `vite build`.
    target: "chrome105",
    minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
});
