import type { AuthExport } from "@liar/protocol";
import { createAuth } from "./session";
import { decodeExport, decodeStart } from "./decode";
import {
  AuthError,
  PROVIDERS,
  type AuthPort,
  type AuthTransport,
} from "./types";
export interface AuthEffects {
  redirect(url: string): void;
  download(data: AuthExport): Promise<void>;
}
export type AuthChannel = Pick<
  BroadcastChannel,
  "postMessage" | "addEventListener" | "removeEventListener" | "close"
>;
function defaultChannel(): AuthChannel | null {
  try {
    return typeof BroadcastChannel === "undefined"
      ? null
      : new BroadcastChannel("liar.auth.v1");
  } catch {
    return null;
  }
}
export function createBrowserAuth(
  browser: Window,
  transport: AuthTransport,
  channel: AuthChannel | null = defaultChannel(),
): { auth: AuthPort; effects: AuthEffects; dispose(): void } {
  const auth = createAuth(
    transport,
    () => browser.navigator.onLine,
    () => channel?.postMessage("invalidate"),
  );
  const refresh = () => {
    void auth.refresh();
  };
  const offline = () => auth.invalidate();
  const visible = () => {
    if (browser.document.visibilityState === "visible") refresh();
  };
  const restored = (event: Event) => {
    if ((event as PageTransitionEvent).persisted) refresh();
  };
  const message = (event: Event) => {
    if ((event as MessageEvent<unknown>).data === "invalidate") {
      auth.invalidate();
      refresh();
    }
  };
  browser.addEventListener("offline", offline);
  browser.addEventListener("online", refresh);
  browser.addEventListener("pageshow", restored);
  browser.document.addEventListener("visibilitychange", visible);
  channel?.addEventListener("message", message);
  const effects: AuthEffects = {
    redirect(url) {
      const provider = PROVIDERS.find((p) => url.startsWith(p.origin));
      if (!provider) throw new AuthError("auth_invalid");
      browser.location.assign(
        decodeStart({ authorize_url: url }, provider.id).authorize_url,
      );
    },
    async download(data) {
      const minimal = decodeExport(data);
      const url = URL.createObjectURL(
        new Blob([JSON.stringify(minimal, null, 2)], {
          type: "application/json",
        }),
      );
      try {
        const link = browser.document.createElement("a");
        link.href = url;
        link.download = "liar-account.json";
        link.click();
      } finally {
        setTimeout(() => URL.revokeObjectURL(url), 1000);
      }
    },
  };
  return {
    auth,
    effects,
    dispose() {
      browser.removeEventListener("offline", offline);
      browser.removeEventListener("online", refresh);
      browser.removeEventListener("pageshow", restored);
      browser.document.removeEventListener("visibilitychange", visible);
      channel?.removeEventListener("message", message);
      channel?.close();
      auth.dispose();
    },
  };
}
