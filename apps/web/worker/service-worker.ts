import { clientsClaim, setCacheNameDetails } from "workbox-core";
import { precacheAndRoute, createHandlerBoundToURL } from "workbox-precaching";
import { registerRoute, NavigationRoute } from "workbox-routing";
import {
  coordinateIdleClients,
  type UpdateClient,
} from "../src/services/update-coordinator";
declare const self: ServiceWorkerGlobalScope & {
  __WB_MANIFEST: Array<{ url: string; revision: string | null }>;
};
setCacheNameDetails({ prefix: "liar-static", suffix: "v1" });
precacheAndRoute(self.__WB_MANIFEST);
registerRoute(
  new NavigationRoute(createHandlerBoundToURL("/index.html"), {
    allowlist: [
      /^\/(?:\?.*)?$/,
      /^\/(?:en|ko|ja|zh-CN|es|pt-BR|de|fr)(?:\/[^?]*)?(?:\?.*)?$/,
    ],
  }),
);
clientsClaim();
const inScope = (url: string) => url.startsWith(self.registration.scope);
const windows = async () =>
  (
    await self.clients.matchAll({ type: "window", includeUncontrolled: true })
  ).filter((client) => inScope(client.url));
function port(client: Client): UpdateClient {
  return {
    id: client.id,
    release: (token) => client.postMessage({ type: "release", token }),
    prepare: (token) =>
      new Promise((resolve) => {
        const channel = new MessageChannel();
        let settled = false;
        const done = (value: boolean) => {
          if (settled) return;
          settled = true;
          clearTimeout(timer);
          channel.port1.close();
          channel.port2.close();
          resolve(value);
        };
        const timer = setTimeout(() => done(false), 2000);
        channel.port1.onmessage = (event) => done(event.data === true);
        channel.port1.onmessageerror = () => done(false);
        try {
          client.postMessage({ type: "prepare", token }, [channel.port2]);
        } catch {
          done(false);
        }
      }),
  };
}
let working = false;
self.addEventListener("message", (event) => {
  const type: unknown = event.data?.type;
  if ((type !== "update" && type !== "clear-cache") || !event.ports[0]) return;
  const reply = event.ports[0];
  event.waitUntil(
    (async () => {
      const source = event.source;
      if (!source || !("id" in source) || working) {
        reply.postMessage(false);
        reply.close();
        return;
      }
      const client = await self.clients.get(source.id);
      if (
        working ||
        !client ||
        client.type !== "window" ||
        !inScope(client.url)
      ) {
        reply.postMessage(false);
        reply.close();
        return;
      }
      working = true;
      const token = crypto.randomUUID();
      let prepared: UpdateClient[] = [];
      const result = await coordinateIdleClients({
        clients: async () => {
          prepared = (await windows()).map(port);
          return prepared;
        },
        token: () => token,
        now: () => performance.now(),
        commit: async () => {
          if (type === "update") {
            await self.skipWaiting();
            return;
          }
          if (self.registration.installing) throw Error("installing");
          for (const client of await windows())
            client.postMessage({ type: "cache-disabled" });
          if (!(await self.registration.unregister()))
            throw Error("unregister_failed");
          for (const key of await caches.keys())
            if (key.startsWith("liar-static-")) await caches.delete(key);
          for (const client of prepared) client.release(token);
        },
      });
      working = false;
      reply.postMessage(result);
      reply.close();
    })(),
  );
});
