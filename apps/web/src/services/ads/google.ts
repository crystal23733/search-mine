import type { AutoAdsScriptPort } from "./types";
export interface GooglePublisherClient extends AutoAdsScriptPort {
  prepare(signal: AbortSignal): Promise<void>;
  prepared(): boolean;
  state(): "disabled" | "idle" | "loading" | "ready" | "error";
  subscribe(listener: () => void): () => void;
  pause(): void;
  dispose(): void;
}
type Namespace = unknown[] & { pauseAdRequests?: number };
export function createGooglePublisherClient(
  document: Document,
  enabled: boolean,
  publisherId?: unknown,
): GooglePublisherClient {
  const publisher =
    enabled &&
    typeof publisherId === "string" &&
    /^ca-pub-\d{16}$/.test(publisherId)
      ? publisherId
      : null;
  const browser = document.defaultView as
    (Window & { adsbygoogle?: unknown }) | null;
  const listeners = new Set<() => void>();
  const abortRemovers = new Set<() => void>();
  let state: ReturnType<GooglePublisherClient["state"]> =
    publisher && browser ? "idle" : "disabled";
  let attempted = false,
    disposed = false;
  let namespace: Namespace | undefined;
  let pending: Promise<void> | undefined;
  let cancel: (() => void) | undefined;
  const emit = () => {
    if (!disposed) for (const listener of listeners) listener();
  };
  const pause = () => {
    try {
      const current = browser?.adsbygoogle;
      if (attempted && current && typeof current === "object")
        namespace = current as Namespace;
      if (namespace) namespace.pauseAdRequests = 1;
    } catch {
      state = "error";
      emit();
    }
  };
  function prepare(signal: AbortSignal): Promise<void> {
    if (
      !publisher ||
      !browser ||
      disposed ||
      signal.aborted ||
      state === "error"
    )
      return Promise.reject(new Error("unavailable"));
    if (pending) return pending;
    attempted = true;
    let resolve!: () => void, reject!: (reason: Error) => void;
    pending = new Promise<void>((yes, no) => {
      resolve = yes;
      reject = no;
    });
    const node = document.createElement("script");
    let timer: ReturnType<typeof setTimeout> | undefined;
    let settled = false;
    const finish = (success: boolean) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      signal.removeEventListener("abort", failed);
      node.removeEventListener("load", loaded);
      node.removeEventListener("error", failed);
      cancel = undefined;
      state = success ? "ready" : "error";
      if (success) resolve();
      else {
        pause();
        node.remove();
        reject(new Error("unavailable"));
      }
      emit();
    };
    const loaded = () => finish(true),
      failed = () => finish(false);
    cancel = failed;
    try {
      if (
        document.querySelector(
          "script[data-liar-auto-ads],script[src*='pagead/js/adsbygoogle.js']",
        )
      )
        throw new Error("unavailable");
      if (
        browser.adsbygoogle !== undefined &&
        !Array.isArray(browser.adsbygoogle)
      )
        throw new Error("unavailable");
      namespace = (browser.adsbygoogle ?? []) as Namespace;
      // Pause before insertion: the same tag supplies messaging and later Auto ads.
      namespace.pauseAdRequests = 1;
      browser.adsbygoogle = namespace;
      node.async = true;
      node.crossOrigin = "anonymous";
      node.referrerPolicy = "strict-origin";
      node.nonce =
        document.querySelector<HTMLScriptElement>("script[nonce]")?.nonce ?? "";
      node.dataset.liarAutoAds = "google-messaging";
      node.src = `https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js?client=${publisher}`;
      node.addEventListener("load", loaded, { once: true });
      node.addEventListener("error", failed, { once: true });
      signal.addEventListener("abort", failed, { once: true });
      timer = setTimeout(failed, 3000);
      state = "loading";
      emit();
      if (signal.aborted) failed();
      else document.head.append(node);
    } catch {
      failed();
    }
    return pending;
  }
  return {
    configured: () => publisher !== null && browser !== null,
    prepared: () => attempted,
    state: () => state,
    prepare,
    async load(signal) {
      await prepare(signal);
      if (disposed || signal.aborted || state !== "ready" || !namespace)
        throw new Error("unavailable");
      const current = browser?.adsbygoogle;
      if (!current || typeof current !== "object")
        throw new Error("unavailable");
      namespace = current as Namespace;
      namespace.pauseAdRequests = 0;
      const abort = () => {
        pause();
        signal.removeEventListener("abort", abort);
        abortRemovers.delete(remove);
      };
      const remove = () => signal.removeEventListener("abort", abort);
      abortRemovers.add(remove);
      signal.addEventListener("abort", abort, { once: true });
      if (signal.aborted) abort();
    },
    pause,
    subscribe(listener) {
      if (!disposed) listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    dispose() {
      if (disposed) return;
      cancel?.();
      pause();
      disposed = true;
      for (const remove of abortRemovers) remove();
      abortRemovers.clear();
      listeners.clear();
      // Removing a node would not unload an already evaluated SDK.
    },
  };
}
