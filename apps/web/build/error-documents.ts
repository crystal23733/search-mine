import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import type { Plugin, ViteDevServer } from "vite";
import {
  documentStatus,
  isApplicationDocument,
} from "../src/services/document-route.ts";
export function errorDocuments(): Plugin {
  function install(
    server: Pick<ViteDevServer, "config" | "middlewares">,
    transform?: ViteDevServer["transformIndexHtml"],
  ) {
    const file = transform
      ? resolve(server.config.root, "index.html")
      : resolve(server.config.root, server.config.build.outDir, "index.html");
    server.middlewares.use((request, response, next) => {
      const method = request.method ?? "GET",
        accept = request.headers.accept ?? "";
      if (
        !["GET", "HEAD"].includes(method) ||
        !accept
          .split(",")
          .some(
            (type) =>
              type.trim().split(";")[0].trim().toLowerCase() === "text/html",
          )
      ) {
        next();
        return;
      }
      let path: string;
      try {
        path = new URL(request.url ?? "/", "http://local.invalid").pathname;
      } catch {
        next();
        return;
      }
      const status = documentStatus(path);
      if (status === 200 || !isApplicationDocument(path)) {
        next();
        return;
      }
      void (async () => {
        // Only the configured entry file is read; the request never becomes a filesystem path.
        const source = await readFile(file, "utf8");
        const html = transform
          ? await transform(request.url ?? "/", source)
          : source;
        if (response.destroyed) return;
        response.statusCode = status;
        response.setHeader("content-type", "text/html; charset=utf-8");
        response.setHeader("cache-control", "no-store");
        response.setHeader("x-content-type-options", "nosniff");
        response.setHeader("referrer-policy", "no-referrer");
        response.setHeader("x-robots-tag", "noindex, noarchive");
        response.end(method === "HEAD" ? undefined : html);
      })().catch(next);
    });
  }
  return {
    name: "liar-error-documents",
    configureServer(server) {
      install(server, (url, html) => server.transformIndexHtml(url, html));
    },
    configurePreviewServer(server) {
      install(server);
    },
  };
}
