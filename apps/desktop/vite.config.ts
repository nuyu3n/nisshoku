import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Set by `tauri android dev` / `tauri ios dev` so that the device can reach the dev server.
const host = process.env.TAURI_DEV_HOST;

// https://v2.tauri.app/start/frontend/vite/
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  build: { chunkSizeWarningLimit: 2000 },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
});
