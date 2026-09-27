import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { enginePlugin } from "./engine-plugin.ts";

export default defineConfig({
  plugins: [react(), enginePlugin()],
  publicDir: "../assets/gui",
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**", "**/.local/**"] } },
  build: { target: "es2021" },
  test: { environment: "jsdom", setupFiles: "./src/test/setup.ts" },
});
