import { defineConfig } from "vite";
import preact from "@preact/preset-vite";
import { VitePWA } from "vite-plugin-pwa";

export default defineConfig({
  plugins: [
    preact(),
    VitePWA({
      strategies: "injectManifest",
      srcDir: "worker",
      filename: "service-worker.ts",
      manifest: false,
      injectRegister: false,
      injectManifest: {
        rollupFormat: "iife",
        globPatterns: ["**/*.{js,css,html,wasm}"],
        globIgnores: ["**/.vite/**"],
      },
      devOptions: { enabled: false },
    }),
  ],
  build: { manifest: true },
  server: { host: "127.0.0.1", port: 5173, strictPort: true },
});
