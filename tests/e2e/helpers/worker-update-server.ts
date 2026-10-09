import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { resolve, sep, extname } from "node:path";
export interface WorkerUpdateServer {
  origin: string;
  advance(): void;
  close(): Promise<void>;
}
// Serve unchanged production assets, with per-test byte revisions at the stable worker URL.
export async function createWorkerUpdateServer(): Promise<WorkerUpdateServer> {
  const root = resolve("apps/web/dist");
  const producer = await readFile(
    "tests/fixtures/worker-update-producer.js",
    "utf8",
  );
  let revision = 0;
  const types: Record<string, string> = {
    ".html": "text/html",
    ".js": "text/javascript",
    ".css": "text/css",
    ".wasm": "application/wasm",
  };
  const server = createServer((request, response) => {
    void (async () => {
      try {
        const path = decodeURIComponent(
          new URL(request.url ?? "/", "http://127.0.0.1").pathname,
        );
        if (
          !["GET", "HEAD"].includes(request.method ?? "") ||
          path.startsWith("/api/")
        ) {
          response.writeHead(503, { "cache-control": "no-store" }).end();
          return;
        }
        const file = resolve(root, `.${path}`);
        if (!file.startsWith(`${root}${sep}`)) {
          response.writeHead(400).end();
          return;
        }
        const extension = extname(file);
        const asset = extension ? file : resolve(root, "index.html");
        const bytes = await readFile(asset);
        const body =
          path === "/service-worker.js"
            ? Buffer.concat([
                bytes,
                Buffer.from(
                  `\n// fixture revision ${revision}\n` +
                    producer.replace('"fixture-revision"', String(revision)),
                ),
              ])
            : bytes;
        response.writeHead(200, {
          "content-type": types[extname(asset)] ?? "application/octet-stream",
          "cache-control": "no-store",
        });
        response.end(request.method === "HEAD" ? undefined : body);
      } catch {
        response.writeHead(404).end();
      }
    })();
  });
  await new Promise<void>((accept, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", accept);
  });
  const address = server.address();
  if (!address || typeof address === "string") throw Error("fixture address");
  return {
    origin: `http://127.0.0.1:${address.port}`,
    advance: () => {
      revision++;
    },
    close: () =>
      new Promise<void>((accept, reject) => {
        server.close((error) => (error ? reject(error) : accept()));
        server.closeIdleConnections();
      }),
  };
}
