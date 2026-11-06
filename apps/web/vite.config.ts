import { defineConfig, loadEnv } from "vite";
import { publicDocuments } from "./build/public-documents.ts";
import { errorDocuments } from "./build/error-documents.ts";
import preact from "@preact/preset-vite";
import { VitePWA } from "vite-plugin-pwa";

export default defineConfig(({ mode }) => ({
  plugins: [
    publicDocuments(
      loadEnv(mode, process.cwd(), "VITE_").VITE_PUBLIC_SITE_ORIGIN,
    ),
    errorDocuments(),
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
}));
