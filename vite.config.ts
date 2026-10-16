import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed port and needs to see Rust errors in the terminal.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "es2022", outDir: "dist" },
  // Only the app's own tests, not example specs that tooling keeps elsewhere.
  test: { include: ["src/**/*.test.{ts,tsx}"] },
});
