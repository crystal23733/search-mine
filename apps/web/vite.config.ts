import { precacheDocumentURL } from "./build/precache-documents.ts";
import { readSiteConfig } from "./build/site-config.ts";
import { defineConfig, loadEnv } from "vite";
import { publicDocuments } from "./build/public-documents.ts";
import { errorDocuments } from "./build/error-documents.ts";
import preact from "@preact/preset-vite";
import { VitePWA } from "vite-plugin-pwa";

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), ["VITE_", "LIAR_PUBLIC_"]);
  const site = readSiteConfig(env.LIAR_PUBLIC_CONFIG_FILE);
  if (env.VITE_CMP_PROVIDER && env.VITE_CMP_PROVIDER !== "google")
    throw Error("Unsupported CMP provider");
  if (
    env.VITE_CMP_PROVIDER === "google" &&
    (!site ||
      !env.VITE_PUBLIC_SITE_ORIGIN ||
      !site.google.frame.length ||
      !site.google.connect.length ||
      !site.google.image.length ||
      !/^ca-pub-\d{16}$/.test(env.VITE_ADSENSE_CLIENT_ID ?? ""))
  )
    throw Error(
      "Google messaging requires published policies, CSP origins and a real publisher ID",
    );
  return {
    define: {
      "import.meta.env.VITE_POLICY_READY": JSON.stringify(
        site ? "true" : "false",
      ),
    },
    plugins: [
      publicDocuments(
        env.VITE_PUBLIC_SITE_ORIGIN,
        env.LIAR_PUBLIC_CONFIG_FILE,
        env.VITE_CMP_PROVIDER === "google"
          ? env.VITE_ADSENSE_CLIENT_ID
          : undefined,
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
          globPatterns: [
            "**/*.{js,css,html,wasm,png,webmanifest}",
            ...(site ? ["policies/*.json"] : []),
          ],
          globIgnores: ["**/.vite/**"],
          manifestTransforms: [
            async (entries) => ({
              manifest: entries.map((entry) => ({
                ...entry,
                url: precacheDocumentURL(entry.url),
              })),
              warnings: [],
            }),
          ],
        },
        devOptions: { enabled: false },
      }),
    ],
    build: { manifest: true },
    server: { host: "127.0.0.1", port: 5173, strictPort: true },
  };
});
