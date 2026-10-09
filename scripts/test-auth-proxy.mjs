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
server.on("upgrade", (req, client, head) => {
  if (req.url !== "/api/v1/ws") {
    client.destroy();
    return;
  }
  const upstream = http.request({
    hostname: "127.0.0.1",
    port: 3001,
    path: "/api/v1/ws",
    method: "GET",
    headers: { ...req.headers, host: "localhost:8443" },
  });
  upstream.setTimeout(12000, () => upstream.destroy());
  upstream.on("upgrade", (reply, socket, serverHead) => {
    upstream.setTimeout(0);
    const headers = [];
    for (let i = 0; i < reply.rawHeaders.length; i += 2)
      headers.push(`${reply.rawHeaders[i]}: ${reply.rawHeaders[i + 1]}`);
    client.write(
      `HTTP/1.1 ${reply.statusCode} ${reply.statusMessage}\r\n${headers.join("\r\n")}\r\n\r\n`,
    );
    if (serverHead.length) client.write(serverHead);
    if (head.length) socket.write(head);
    socket.on("error", () => client.destroy());
    client.on("error", () => socket.destroy());
    socket.on("close", () => client.destroy());
    client.on("close", () => socket.destroy());
    socket.pipe(client);
    client.pipe(socket);
  });
  upstream.on("response", () => client.destroy());
  upstream.on("error", () => client.destroy());
  client.on("close", () => upstream.destroy());
  upstream.end();
});
server.listen(8443, "127.0.0.1");
for (const signal of ["SIGTERM", "SIGINT"])
  process.on(signal, () => server.close(() => process.exit(0)));
