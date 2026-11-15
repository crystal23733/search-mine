import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { AuthState } from "../auth/types";
import { createAutoAdsBrowser } from "./browser";

const cleanup: Array<() => void> = [];
beforeEach(() => {
  window.history.replaceState(null, "", "/ko/");
  vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
  vi.spyOn(navigator, "onLine", "get").mockReturnValue(true);
});
afterEach(() => {
  for (const dispose of cleanup.splice(0)) dispose();
  vi.restoreAllMocks();
});
function setup(path = "/ko/", configure = true) {
  window.history.replaceState(null, "", path);
  const listeners = new Set<() => void>();
  const subscribe = (fn: () => void) => {
    listeners.add(fn);
    return () => {
      listeners.delete(fn);
    };
  };
  let account: string | null = null;
  let revision = 1;
  const state: AuthState = {
    status: "ready",
    account: null,
    providers: [],
    identities: [],
    working: false,
    error: null,
    notice: null,
    manualAppleDisconnect: false,
    appleRevokePending: false,
  };
  const choice = {
    status: "valid" as "valid" | "denied",
    analytics: false,
    advertising: true,
    working: false,
    canReopen: true,
  };
  const busy = { busy: false, locked: false };
  let learning: "new" | "complete" = "complete";
  let persistent = true;
  const script = {
    configured: () => configure,
    load: vi.fn<(signal: AbortSignal) => Promise<void>>().mockResolvedValue(),
  };
  const documents = { navigate: vi.fn(), reload: vi.fn() };
  const deps = {
    script,
    documents,
    consent: { read: () => ({ ...choice }), subscribe },
    auth: {
      read: () => state,
      account: () => account,
      revision: () => revision,
      subscribe,
    },
    activity: { read: () => ({ ...busy }), subscribe },
    learning: { read: () => learning, persistent: () => persistent },
    preferences: { persistent: () => persistent, subscribe },
  };
  return {
    deps,
    script,
    documents,
    choice,
    state,
    busy,
    notify: () => {
      for (const fn of [...listeners]) fn();
    },
    owner: () => {
      account = "another-account";
      revision++;
    },
    learning: () => {
      learning = "new";
    },
    memory: () => {
      persistent = false;
    },
    start: () => {
      const app = createAutoAdsBrowser(window, deps);
      cleanup.push(app.dispose);
      return app;
    },
  };
}

test("current advertising permission starts one script in a safe document despite repeated notifications", () => {
  const s = setup();
  s.start();
  expect(s.script.load).toHaveBeenCalledTimes(1);
  s.notify();
  s.notify();
  expect(s.script.load).toHaveBeenCalledTimes(1);
});

test("SDK document navigates before publishing a game route even while loading", () => {
  const s = setup();
  s.script.load.mockImplementation(() => new Promise(() => {}));
  const app = s.start();
  const publish = vi.fn();
  app.navigation.subscribe(publish);
  app.navigation.go("/practice");
  expect(s.documents.navigate).toHaveBeenCalledWith(
    `${location.origin}/ko/practice`,
  );
  expect(location.pathname).toBe("/ko/");
  expect(publish).not.toHaveBeenCalled();
});

test("all eight locales allow only clean home and rules documents", () => {
  for (const locale of ["en", "ko", "ja", "zh-CN", "es", "pt-BR", "de", "fr"]) {
    for (const route of ["", "rules"]) {
      const s = setup(`/${locale}/${route}`);
      const app = s.start();
      expect(s.script.load).toHaveBeenCalledTimes(1);
      app.dispose();
    }
    for (const route of [
      "practice",
      "tutorial",
      "daily",
      "daily/rankings",
      "daily/official",
      "queue",
      "friends",
      "results",
      "me",
      "me/history",
      "me/account",
      "me/privacy",
      "settings",
      "storage",
      "login",
      "onboarding",
      "403",
      "404",
      "unknown",
    ]) {
      const s = setup(`/${locale}/${route}`);
      const app = s.start();
      expect(s.script.load).not.toHaveBeenCalled();
      app.dispose();
    }
  }
  for (const path of [
    "/xx/",
    "/ko/?invite=private",
    "/ko/rules#private",
    "/ko//rules",
  ]) {
    const s = setup(path);
    s.start();
    expect(s.script.load).not.toHaveBeenCalled();
  }
});

test("configuration, purpose, auth readiness, activity, visibility and memory guards never authorize a script", () => {
  const absent = setup("/ko/", false);
  const app = absent.start();
  app.navigation.go("/practice");
  expect(absent.script.load).not.toHaveBeenCalled();
  expect(absent.documents.navigate).not.toHaveBeenCalled();
  app.dispose();
  const cases = [
    (s: ReturnType<typeof setup>) => {
      s.choice.status = "denied";
    },
    (s: ReturnType<typeof setup>) => {
      s.choice.advertising = false;
      s.choice.analytics = true;
    },
    (s: ReturnType<typeof setup>) => {
      s.choice.working = true;
    },
    (s: ReturnType<typeof setup>) => {
      s.state.status = "loading";
    },
    (s: ReturnType<typeof setup>) => {
      s.busy.busy = true;
    },
    (s: ReturnType<typeof setup>) => {
      s.busy.locked = true;
    },
    (s: ReturnType<typeof setup>) => s.memory(),
    (s: ReturnType<typeof setup>) => s.learning(),
    (s: ReturnType<typeof setup>) => {
      s.deps.preferences.persistent = () => false;
    },
    (s: ReturnType<typeof setup>) => {
      s.deps.consent.read = () => {
        throw Error("CMP failed");
      };
    },
  ];
  for (const change of cases) {
    const s = setup();
    change(s);
    const active = s.start();
    expect(s.script.load).not.toHaveBeenCalled();
    active.dispose();
  }
  const hidden = setup();
  vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
  hidden.start();
  expect(hidden.script.load).not.toHaveBeenCalled();
  vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
  document.dispatchEvent(new Event("visibilitychange"));
  expect(hidden.script.load).toHaveBeenCalledTimes(1);
});

test("auth check completion can start ads, while initial offline and memory-only browsing stay ad-free", () => {
  const s = setup();
  s.state.status = "loading";
  s.start();
  expect(s.script.load).not.toHaveBeenCalled();
  s.state.status = "ready";
  s.notify();
  expect(s.script.load).toHaveBeenCalledTimes(1);
  const offline = setup();
  vi.spyOn(navigator, "onLine", "get").mockReturnValue(false);
  const app = offline.start();
  app.navigation.go("/practice");
  expect(offline.script.load).not.toHaveBeenCalled();
  expect(offline.documents.navigate).not.toHaveBeenCalled();
  app.dispose();
});

test("a protected or query-bearing document needs a fresh safe document before any SDK activation", () => {
  for (const path of ["/ko/practice", "/ko/?invite=private"]) {
    const s = setup(path);
    const app = s.start();
    app.navigation.go("/rules", { invite: null });
    expect(s.script.load).not.toHaveBeenCalled();
    expect(s.documents.navigate).toHaveBeenCalledWith(
      `${location.origin}/ko/rules`,
    );
    app.dispose();
  }
  const s = setup();
  s.choice.advertising = false;
  const app = s.start();
  app.navigation.go("/practice");
  app.navigation.go("/");
  s.choice.advertising = true;
  s.notify();
  expect(s.script.load).not.toHaveBeenCalled();
  expect(s.documents.reload).toHaveBeenCalledTimes(1);
});

test("safe SPA locale changes keep one SDK but popstate cannot publish a protected route", () => {
  const s = setup();
  const app = s.start();
  const publish = vi.fn();
  app.navigation.subscribe(publish);
  app.navigation.go("/rules");
  app.navigation.locale("fr");
  expect(location.pathname).toBe("/fr/rules");
  expect(s.script.load).toHaveBeenCalledTimes(1);
  expect(publish).toHaveBeenCalledTimes(2);
  window.history.replaceState(null, "", "/fr/queue");
  window.dispatchEvent(new PopStateEvent("popstate"));
  expect(s.documents.navigate).toHaveBeenCalledWith(
    `${location.origin}/fr/queue`,
  );
  expect(publish).toHaveBeenCalledTimes(2);
});

test("revocation, account changes, offline and unexpected gameplay retire an SDK document once", () => {
  for (const reason of ["consent", "owner", "offline", "game"] as const) {
    vi.spyOn(navigator, "onLine", "get").mockReturnValue(true);
    const s = setup();
    const app = s.start();
    if (reason === "consent") s.choice.advertising = false;
    if (reason === "owner") s.owner();
    if (reason === "offline")
      vi.spyOn(navigator, "onLine", "get").mockReturnValue(false);
    if (reason === "game") s.busy.busy = true;
    s.notify();
    s.notify();
    expect(s.documents.reload).toHaveBeenCalledTimes(1);
    expect(s.script.load.mock.calls[0][0].aborted).toBe(true);
    app.navigation.go("/practice");
    expect(s.documents.navigate).not.toHaveBeenCalled();
    app.dispose();
  }
});

test("hidden and safe-update locks do not force an active SDK reload; restored SDK documents do", () => {
  const s = setup();
  const app = s.start();
  s.busy.locked = true;
  s.notify();
  vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
  document.dispatchEvent(new Event("visibilitychange"));
  expect(s.documents.reload).not.toHaveBeenCalled();
  window.dispatchEvent(
    Object.assign(new Event("pageshow"), { persisted: true }),
  );
  expect(s.documents.reload).toHaveBeenCalledTimes(1);
  app.dispose();
});

test("provider failures never permit unsafe SPA fallback or retry, and late completion cannot reopen a retired document", async () => {
  for (const mode of ["sync", "reject", "pending"] as const) {
    const s = setup();
    let finish: () => void = () => {};
    s.script.load.mockImplementation(() => {
      if (mode === "sync") throw Error("blocked");
      if (mode === "reject") return Promise.reject(Error("blocked"));
      return new Promise<void>((resolve) => {
        finish = resolve;
      });
    });
    const app = s.start();
    await Promise.resolve();
    s.notify();
    s.documents.navigate.mockImplementation(() => {
      throw Error("navigation denied");
    });
    app.navigation.go("/practice");
    finish();
    await Promise.resolve();
    s.notify();
    expect(s.script.load).toHaveBeenCalledTimes(1);
    expect(location.pathname).toBe("/ko/");
    expect(s.documents.navigate).toHaveBeenCalledTimes(1);
    app.dispose();
  }
});

test("dispose cancels pending work, removes subscriptions and keeps document cleanup idempotent", () => {
  const s = setup();
  const app = s.start();
  app.dispose();
  app.dispose();
  const calls = s.documents.reload.mock.calls.length;
  s.notify();
  window.dispatchEvent(new Event("online"));
  document.dispatchEvent(new Event("visibilitychange"));
  expect(s.documents.reload).toHaveBeenCalledTimes(calls);
  expect(s.script.load.mock.calls[0][0].aborted).toBe(true);
  expect(s.script.load).toHaveBeenCalledTimes(1);
});

test("a previously retiring document resets its navigation barrier on BFCache restoration", () => {
  const s = setup();
  const app = s.start();
  app.navigation.go("/practice");
  expect(s.documents.navigate).toHaveBeenCalledTimes(1);
  window.dispatchEvent(
    Object.assign(new Event("pageshow"), { persisted: true }),
  );
  expect(s.documents.reload).toHaveBeenCalledTimes(1);
  const protectedPage = setup("/ko/practice");
  const other = protectedPage.start();
  other.navigation.go("/rules");
  expect(protectedPage.documents.navigate).toHaveBeenCalledTimes(1);
  window.dispatchEvent(
    Object.assign(new Event("pageshow"), { persisted: true }),
  );
  other.navigation.go("/friends");
  expect(location.pathname).toBe("/ko/friends");
});

test("fixed public skip anchors preserve keyboard navigation without reloading", () => {
  const s = setup("/ko/rules#main");
  s.start();
  expect(s.script.load).toHaveBeenCalledTimes(1);
  window.dispatchEvent(new HashChangeEvent("hashchange"));
  expect(s.documents.reload).not.toHaveBeenCalled();
});

test("unknown fragment changes retire a previously clean SDK document", () => {
  const s = setup();
  s.start();
  history.replaceState(null, "", "/ko/#private");
  window.dispatchEvent(new HashChangeEvent("hashchange"));
  expect(s.documents.reload).toHaveBeenCalledTimes(1);
});

test("the public tutorial guide remains excluded from advertising in every locale", () => {
  for (const locale of ["en", "ko", "ja", "zh-CN", "es", "pt-BR", "de", "fr"]) {
    const s = setup(`/${locale}/learn`);
    const app = s.start();
    s.notify();
    expect(s.script.load).not.toHaveBeenCalled();
    app.dispose();
  }
});
test("an SDK document cannot publish the tutorial guide and a guide cannot activate an SDK on return", () => {
  const safe = setup("/ko/rules");
  const app = safe.start();
  const publish = vi.fn();
  app.navigation.subscribe(publish);
  app.navigation.go("/learn");
  expect(safe.documents.navigate).toHaveBeenCalledWith(
    `${location.origin}/ko/learn`,
  );
  expect(location.pathname).toBe("/ko/rules");
  expect(publish).not.toHaveBeenCalled();
  app.dispose();
  const guide = setup("/ko/learn");
  const current = guide.start();
  current.navigation.go("/");
  expect(guide.documents.navigate).toHaveBeenCalledWith(
    `${location.origin}/ko/`,
  );
  expect(guide.script.load).not.toHaveBeenCalled();
});

test("paused messaging bootstraps without ad permission and leaves a protected route before publishing", () => {
  const s = setup("/ko/rules");
  s.choice.status = "denied";
  s.choice.advertising = false;
  let prepared = false;
  const messaging = {
    eligible: (url: URL) => url.pathname === "/ko/rules",
    prepared: () => prepared,
    prepare: vi.fn(async () => {
      prepared = true;
    }),
  };
  const app = createAutoAdsBrowser(window, { ...s.deps, messaging });
  cleanup.push(app.dispose);
  expect(messaging.prepare).toHaveBeenCalledTimes(1);
  expect(s.script.load).not.toHaveBeenCalled();
  const publish = vi.fn();
  app.navigation.subscribe(publish);
  app.navigation.go("/me/privacy");
  expect(s.documents.navigate).toHaveBeenCalledWith(
    `${location.origin}/ko/me/privacy`,
  );
  expect(publish).not.toHaveBeenCalled();
});
test("consent document can prepare messaging but never activate Auto ads", () => {
  const s = setup("/ko/consent");
  let prepared = false;
  const messaging = {
    eligible: (url: URL) => url.pathname === "/ko/consent",
    prepared: () => prepared,
    prepare: vi.fn(async () => {
      prepared = true;
    }),
  };
  const app = createAutoAdsBrowser(window, { ...s.deps, messaging });
  cleanup.push(app.dispose);
  expect(messaging.prepare).toHaveBeenCalledTimes(1);
  expect(s.script.load).not.toHaveBeenCalled();
});
test("reopening active Google choices navigates to the isolated management document", () => {
  const s = setup("/ko/rules");
  const app = createAutoAdsBrowser(window, {
    ...s.deps,
    messaging: {
      eligible: () => true,
      prepared: () => true,
      prepare: async () => {},
    },
  });
  cleanup.push(app.dispose);
  s.choice.working = true;
  s.notify();
  expect(s.documents.navigate).toHaveBeenCalledWith(
    `${location.origin}/ko/consent#manage`,
  );
  expect(s.documents.reload).not.toHaveBeenCalled();
});
