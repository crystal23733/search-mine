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
      manifest: {
        id: "/",
        scope: "/",
        start_url: "/",
        name: "Liar Sweeper",
        short_name: "Liar Sweeper",
        description: "Logic, deception and daily Minesweeper challenges.",
        lang: "en",
        display: "standalone",
        background_color: "#0b1015",
        theme_color: "#0b1015",
        prefer_related_applications: false,
        categories: ["games"],
        icons: [
          {
            src: "/icons/pwa-192x192.png",
            sizes: "192x192",
            type: "image/png",
            purpose: "any",
          },
          {
            src: "/icons/pwa-512x512.png",
            sizes: "512x512",
            type: "image/png",
            purpose: "any",
          },
          {
            src: "/icons/maskable-icon-512x512.png",
            sizes: "512x512",
            type: "image/png",
            purpose: "maskable",
          },
        ],
      },
      includeAssets: ["icons/*.png"],
      injectRegister: false,
      injectManifest: {
        rollupFormat: "iife",
        globPatterns: ["**/*.{js,css,html,wasm,png,webmanifest}"],
        globIgnores: ["**/.vite/**"],
      },
      devOptions: { enabled: false },
    }),
  ],
  build: { manifest: true },
  server: { host: "127.0.0.1", port: 5173, strictPort: true },
}));
