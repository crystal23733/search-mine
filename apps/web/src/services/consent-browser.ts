import { createConsent, type CmpPort, type ConsentPort } from "./consent";
import type { AuthState } from "./auth/types";
import { createConsentGuard, type ConsentContextPort } from "./consent-guard";
export interface ConsentAccountPort {
  read(): Pick<AuthState, "status" | "account" | "working">;
  revision(): number;
  subscribe(listener: () => void): () => void;
}
export type ConsentChannel = Pick<
  BroadcastChannel,
  "postMessage" | "addEventListener" | "removeEventListener" | "close"
>;
export interface ConsentAdapter extends CmpPort {
  dispose(): void;
}
export type ConsentAdapterFactory = (
  context: ConsentContextPort,
) => ConsentAdapter;
export function createConsentContext(
  browser: Window,
  auth: ConsentAccountPort,
  channel: ConsentChannel | null,
): ConsentContextPort & { broadcast(): void; dispose(): void } {
  let disposed = false,
    broken = !channel;
  let token: object = Object.freeze({});
  let identity:
    { ready: boolean; accountId: string | null; revision: number } | undefined;
  const listeners = new Set<() => void>();
  const removers: Array<() => void> = [];
  const emit = () => {
    if (!disposed) for (const listener of listeners) listener();
  };
  function capture() {
    let next = { ready: false, accountId: null as string | null, revision: -1 };
    try {
      const state = auth.read();
      const revision = auth.revision();
      next = {
        ready:
          !disposed &&
          !broken &&
          state.status === "ready" &&
          !state.working &&
          Number.isSafeInteger(revision) &&
          revision >= 0 &&
          browser.navigator.onLine &&
          browser.document.visibilityState === "visible",
        accountId: state.account?.id ?? null,
        revision,
      };
    } catch {
      /* Missing current authority keeps optional purposes blocked. */
    }
    if (
      !identity ||
      identity.ready !== next.ready ||
      identity.accountId !== next.accountId ||
      identity.revision !== next.revision
    ) {
      token = Object.freeze({});
      identity = next;
    }
    return next.ready ? token : null;
  }
  const invalidate = () => {
    capture();
    token = Object.freeze({});
    emit();
  };
  const unavailable = () => {
    broken = true;
    invalidate();
  };
  const message = (event: Event) => {
    if ((event as MessageEvent<unknown>).data === "invalidate") invalidate();
  };
  const restored = (event: Event) => {
    if ((event as PageTransitionEvent).persisted) invalidate();
  };
  const authChanged = () => {
    capture();
    emit();
  };
  function listen(target: EventTarget, type: string, listener: EventListener) {
    target.addEventListener(type, listener);
    removers.push(() => target.removeEventListener(type, listener));
  }
  try {
    const remove = auth.subscribe(authChanged);
    if (typeof remove !== "function") throw Error("unavailable");
    removers.push(remove);
    listen(browser, "offline", invalidate);
    listen(browser, "online", invalidate);
    listen(browser, "pagehide", invalidate);
    listen(browser, "pageshow", restored);
    listen(browser.document, "visibilitychange", invalidate);
    if (channel) {
      channel.addEventListener("message", message);
      removers.push(() => channel.removeEventListener("message", message));
      channel.addEventListener("messageerror", unavailable);
      removers.push(() =>
        channel.removeEventListener("messageerror", unavailable),
      );
    }
  } catch {
    broken = true;
    for (const remove of removers.splice(0))
      try {
        remove();
      } catch {
        /* Partial setup stays closed. */
      }
  }
  return {
    capture,
    invalidate,
    subscribe(listener) {
      if (!disposed) listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    broadcast() {
      if (disposed || broken) return;
      try {
        channel!.postMessage("invalidate");
      } catch {
        unavailable();
      }
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      token = Object.freeze({});
      listeners.clear();
      for (const remove of removers)
        try {
          remove();
        } catch {
          /* Disposal stays closed. */
        }
      try {
        channel?.close();
      } catch {
        /* No state is retained after disposal. */
      }
    },
  };
}
function defaultChannel(): ConsentChannel | null {
  try {
    return typeof BroadcastChannel === "undefined"
      ? null
      : new BroadcastChannel("liar.consent.v1");
  } catch {
    return null;
  }
}
export function createBrowserConsent(
  browser: Window,
  auth: ConsentAccountPort,
  factory?: ConsentAdapterFactory,
  channel?: ConsentChannel | null,
): ConsentPort {
  if (!factory) return createConsent();
  const context = createConsentContext(
    browser,
    auth,
    channel === undefined ? defaultChannel() : channel,
  );
  let adapter: ConsentAdapter;
  try {
    adapter = factory(context);
  } catch {
    context.dispose();
    return createConsent();
  }
  const guard = createConsentGuard(
    {
      read: () => adapter.read(),
      subscribe: (listener) =>
        adapter.subscribe(() => {
          context.broadcast();
          listener();
        }),
      reopen: (signal) => adapter.reopen(signal),
    },
    context,
  );
  const consent = createConsent(guard);
  let disposed = false;
  return {
    ...consent,
    dispose() {
      if (disposed) return;
      disposed = true;
      consent.dispose();
      guard.dispose();
      context.dispose();
      try {
        adapter.dispose();
      } catch {
        /* Disposed permissions remain blocked. */
      }
    },
  };
}
