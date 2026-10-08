// Test-only fixed loopback HTTPS proxy. No arbitrary upstream, TLS key or auth bypass in production.
import https from "node:https";
import http from "node:http";
import { readFileSync } from "node:fs";
const fixtures = new URL("../tests/fixtures/https/", import.meta.url);
const server = https.createServer(
  {
    key: readFileSync(new URL("localhost-test-only.key", fixtures)),
    cert: readFileSync(new URL("localhost-test-only.pem", fixtures)),
  },
  (req, res) => {
    const pathname = new URL(req.url, "https://localhost:8443").pathname;
    const backend =
      pathname.startsWith("/api/") || pathname.startsWith("/__fixture/");
    const upstream = http.request(
      {
        hostname: "127.0.0.1",
        port: backend ? 3001 : 4173,
        path: req.url,
        method: req.method,
        headers: {
          ...req.headers,
          host: backend ? "localhost:8443" : "127.0.0.1:4173",
        },
      },
      (reply) => {
        res.writeHead(reply.statusCode, reply.headers);
        reply.pipe(res);
      },
    );
    upstream.setTimeout(12000, () => upstream.destroy());
    upstream.on("error", () => {
      if (!res.headersSent) res.writeHead(503, { "cache-control": "no-store" });
      res.end();
    });
    req.pipe(upstream);
  },
);
server.listen(8443, "127.0.0.1");
for (const signal of ["SIGTERM", "SIGINT"])
  process.on(signal, () => server.close(() => process.exit(0)));
