import { defineConfig, devices } from "@playwright/test";

// The popup's pages with fixture data (demo.html), served by Vite: no Tauri, no backend.
export default defineConfig({
  testDir: "e2e",
  use: { baseURL: "http://localhost:1420", ...devices["Desktop Chrome"], channel: "chrome", viewport: { width: 360, height: 900 } },
  webServer: { command: "npx vite --port 1420 --strictPort", url: "http://localhost:1420/demo.html", reuseExistingServer: true },
});
