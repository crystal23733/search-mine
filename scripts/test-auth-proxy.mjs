// Test-only fixed loopback HTTPS proxy. No arbitrary upstream, TLS key or auth bypass in production.
import https from "node:https";
import http from "node:http";
import { readFileSync } from "node:fs";
const fixtures = new URL("../tests/fixtures/https/", import.meta.url);
let accepted = 0, serverEnded = 0, forced = 0;
const pending = new Set();
const server = https.createServer(
  {
    key: readFileSync(new URL("localhost-test-only.key", fixtures)),
    cert: readFileSync(new URL("localhost-test-only.pem", fixtures)),
  },
  (req, res) => {
    if (req.method === "GET" && req.url === "/__fixture/transport") {
      res.writeHead(200, {"content-type":"application/json", "cache-control":"no-store"});
      res.end(JSON.stringify({accepted, server_ended:serverEnded, pending:pending.size, forced}));
      return;
    }
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
  let upgraded = false;
  const upstream = http.request({
    hostname: "127.0.0.1",
    port: 3001,
    path: "/api/v1/ws",
    method: "GET",
    headers: { ...req.headers, host: "localhost:8443" },
  });
  upstream.setTimeout(12000, () => upstream.destroy());
  upstream.on("upgrade", (reply, socket, serverHead) => {
    upgraded = true;
    accepted += 1;
    pending.add(socket);
    let ended = false, cleanup;
    const closeWrite = () => {
      if (ended || socket.destroyed || cleanup) return;
      socket.end();
      cleanup = setTimeout(() => socket.destroy(), 2000);
      cleanup.unref();
    };
    socket.on("end", () => {
      ended = true;
      serverEnded += 1;
      pending.delete(socket);
      clearTimeout(cleanup);
    });
    socket.on("close", () => {
      clearTimeout(cleanup);
      if (!ended) forced += 1;
      pending.delete(socket);
      client.destroy();
    });
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
    client.on("error", closeWrite);
    client.on("close", closeWrite);
    socket.pipe(client);
    client.pipe(socket);
  });
  upstream.on("response", () => client.destroy());
  upstream.on("error", () => client.destroy());
  client.on("close", () => { if (!upgraded) upstream.destroy(); });
  upstream.end();
});
server.listen(8443, "127.0.0.1");
for (const signal of ["SIGTERM", "SIGINT"])
  process.on(signal, () => server.close(() => process.exit(0)));
