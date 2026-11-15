import { createNavigation } from "../navigation";
import { isLocale } from "../locale";
import type { AutoAdsDependencies } from "./types";

export function createAutoAdsBrowser(
  browser: Window,
  deps: AutoAdsDependencies,
) {
  let disposed = false,
    started = false,
    retiring = false;
  let controller: AbortController | undefined;
  let owner = "";
  let configured = false;
  try {
    configured = deps.script.configured();
  } catch {
    /* Missing providers stay disabled. */
  }
  const documents = deps.documents ?? {
    navigate: (url: string) => browser.location.assign(url),
    reload: () => browser.location.reload(),
  };
  const publicDocument = (url: URL) => {
    const parts = url.pathname.split("/");
    return (
      url.origin === browser.location.origin &&
      !url.search &&
      (!url.hash || url.hash === "#main") &&
      isLocale(parts[1]) &&
      (parts.slice(2).join("/") === "" || parts.slice(2).join("/") === "rules")
    );
  };
  const messagingDocument = (url: URL) =>
    deps.messaging?.eligible(url) ?? false;
  const permittedDocument = (url: URL) =>
    publicDocument(url) || messagingDocument(url);
  let cleanDocument =
    permittedDocument(new URL(browser.location.href)) &&
    (deps.messaging?.documentReady?.() ?? true);
  let preparing = false;
  const dirty = () =>
    started || preparing || Boolean(deps.messaging?.prepared());
  const gate = (url: URL) => {
    try {
      const consent = deps.consent.read(),
        activity = deps.activity.read();
      const identity = `${deps.auth.account() ?? ""}/${deps.auth.revision()}`;
      const authorized =
        publicDocument(url) &&
        browser.navigator.onLine &&
        consent.status === "valid" &&
        consent.advertising &&
        !consent.working &&
        deps.auth.read().status === "ready" &&
        !activity.busy;
      return {
        authorized,
        managing: consent.working,
        identity,
        canStart:
          authorized &&
          !activity.locked &&
          browser.document.visibilityState === "visible" &&
          deps.learning.persistent() &&
          deps.preferences.persistent() &&
          (url.pathname.endsWith("/rules") || deps.learning.read() !== "new"),
      };
    } catch {
      return { authorized: false, canStart: false, identity: "" };
    }
  };
  const retire = (url?: URL) => {
    if (retiring) return;
    retiring = true;
    controller?.abort();
    try {
      if (url) documents.navigate(url.href);
      else documents.reload();
    } catch {
      /* Keep the safe view; never publish a protected route in this document. */
    }
  };
  const navigation = createNavigation(browser, {
    beforeChange(url) {
      if (!configured) return true;
      if (disposed || retiring || url.origin !== browser.location.origin)
        return false;
      const current = gate(url);
      if (started && (!current.authorized || current.identity !== owner)) {
        retire(url);
        return false;
      }
      if (dirty() && !permittedDocument(url)) {
        retire(url);
        return false;
      }
      if (!permittedDocument(url)) cleanDocument = false;
      if (!cleanDocument && (current.canStart || messagingDocument(url))) {
        retire(url);
        return false;
      }
      return true;
    },
  });
  const evaluate = () => {
    if (!configured || disposed || retiring) return;
    const url = navigation.current();
    if (!permittedDocument(url)) cleanDocument = false;
    try {
      const activity = deps.activity.read();
      if (
        deps.messaging &&
        (deps.messaging.documentReady?.() ?? true) &&
        !preparing &&
        !deps.messaging.prepared() &&
        messagingDocument(url) &&
        browser.navigator.onLine &&
        browser.document.visibilityState === "visible" &&
        deps.auth.read().status === "ready" &&
        !activity.busy &&
        !activity.locked &&
        deps.learning.persistent() &&
        deps.preferences.persistent() &&
        (url.pathname.endsWith("/rules") ||
          url.pathname.endsWith("/consent") ||
          deps.learning.read() !== "new")
      ) {
        if (!cleanDocument) {
          retire();
          return;
        }
        preparing = true;
        controller = new AbortController();
        void deps.messaging.prepare(controller.signal).then(evaluate, () => {});
      }
    } catch {
      /* An unavailable message never blocks essential functionality. */
    }
    const current = gate(url);
    if (started) {
      if (!current.authorized || current.identity !== owner) {
        if (deps.messaging && current.managing)
          retire(new URL(`/${url.pathname.split("/")[1]}/consent#manage`, url));
        else retire();
      }
      return;
    }
    if (!current.canStart) return;
    if (!cleanDocument) {
      retire();
      return;
    }
    // Mark before calling provider code, including synchronous errors and late callbacks.
    started = true;
    owner = current.identity;
    controller = new AbortController();
    try {
      void deps.script.load(controller.signal).catch(() => {});
    } catch {
      /* An optional script failure never reaches game flow. */
    }
  };
  const remove: Array<() => void> = [];
  const restored = (event: Event) => {
    if ((event as PageTransitionEvent).persisted) {
      // A cached document has completed its earlier navigation; this is a new boundary.
      retiring = false;
      if (dirty()) {
        retire();
        return;
      }
    }
    evaluate();
  };
  const hidden = () => {
    controller?.abort();
  };
  if (configured) {
    remove.push(
      navigation.subscribe(evaluate),
      deps.auth.subscribe(evaluate),
      deps.consent.subscribe(evaluate),
      deps.activity.subscribe(evaluate),
      deps.preferences.subscribe(evaluate),
    );
    for (const name of ["online", "offline", "hashchange"]) {
      browser.addEventListener(name, evaluate);
      remove.push(() => browser.removeEventListener(name, evaluate));
    }
    browser.addEventListener("pageshow", restored);
    browser.addEventListener("pagehide", hidden);
    browser.document.addEventListener("visibilitychange", evaluate);
    remove.push(
      () => browser.removeEventListener("pageshow", restored),
      () => browser.removeEventListener("pagehide", hidden),
      () => browser.document.removeEventListener("visibilitychange", evaluate),
    );
    evaluate();
  }
  return {
    navigation,
    dispose() {
      if (disposed) return;
      disposed = true;
      for (const fn of remove) fn();
      controller?.abort();
      navigation.dispose();
      if (dirty()) retire();
    },
  };
}
