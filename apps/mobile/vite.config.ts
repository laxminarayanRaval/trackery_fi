import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Standard Tauri v2 vite setup: fixed port for the dev server, no clearing
// so cargo/tauri output stays visible.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
});
