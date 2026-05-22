import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";
import { fileURLToPath } from "node:url";

// Tauri 2.x dev integration:
// - Tauri sets `TAURI_DEV_HOST` to override the dev server host when needed
//   (e.g. mobile dev). We honour it but default to localhost.
// - `clearScreen: false` keeps Cargo output visible alongside Vite output.
// - The dev server must bind to a fixed port so `tauri.conf.json` `build.devUrl`
//   can point at it.
const host = process.env["TAURI_DEV_HOST"];
const projectRoot = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host ?? false,
    hmr: host !== undefined
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  resolve: {
    alias: {
      "@": path.resolve(projectRoot, "src"),
    },
  },
  build: {
    target: "es2022",
    sourcemap: true,
    outDir: "dist",
    emptyOutDir: true,
  },
});
