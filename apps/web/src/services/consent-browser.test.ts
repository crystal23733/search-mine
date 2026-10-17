import { afterEach, expect, test, vi } from "vitest";
import {
  createConsentContext,
  createBrowserConsent,
  type ConsentAdapter,
  type ConsentAdapterFactory,
  type ConsentAccountPort,
} from "./consent-browser";

const owned: Array<() => void> = [];
afterEach(() => {
  for (const dispose of owned.splice(0)) dispose();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("factory failure releases context observers and a missing channel constructor cannot grant", () => {
  const s = fixture();
  s.context.dispose();
  const channel = Object.assign(new EventTarget(), {
    postMessage: vi.fn(),
    close: vi.fn(),
  });
  const failed = createBrowserConsent(
    window,
    s.auth,
    () => {
      throw Error("private");
    },
    channel,
  );
  owned.push(() => failed.dispose());
  expect(failed.read()).toMatchObject({
    status: "unavailable",
    analytics: false,
  });
  expect(s.subscribers.size).toBe(0);
  expect(channel.close).toHaveBeenCalledOnce();
  vi.stubGlobal("BroadcastChannel", undefined);
  const consent = createBrowserConsent(window, s.auth, (context) => ({
    read: () => ({
      status: "valid",
      analytics: true,
      advertising: true,
      context: context.capture(),
      revision: 1,
      validFrom: 0,
      expiresAt: Date.now() + 60000,
    }),
    subscribe: () => () => {},
    reopen: async () => {},
    dispose() {},
  }));
  owned.push(() => consent.dispose());
  expect(consent.read()).toMatchObject({
    analytics: false,
    advertising: false,
  });
});

test("partial channel subscription failure releases auth and attached browser observers", () => {
  const s = fixture();
  s.context.dispose();
  const channel = Object.assign(new EventTarget(), {
    postMessage: vi.fn(),
    close: vi.fn(),
  });
  vi.spyOn(channel, "addEventListener").mockImplementation(() => {
    throw Error("private");
  });
  const context = createConsentContext(window, s.auth, channel);
  owned.push(() => context.dispose());
  expect(context.capture()).toBeNull();
  expect(s.subscribers.size).toBe(0);
});
function fixture() {
  let revision = 1;
  let state: ReturnType<ConsentAccountPort["read"]> = {
    status: "ready",
    account: null,
    working: false,
  };
  const subscribers = new Set<() => void>();
  const auth: ConsentAccountPort = {
    read: () => state,
    revision: () => revision,
    subscribe(listener) {
      subscribers.add(listener);
      return () => {
        subscribers.delete(listener);
      };
    },
  };
  const channel = Object.assign(new EventTarget(), {
    postMessage: vi.fn(),
    close: vi.fn(),
  });
  const context = createConsentContext(window, auth, channel);
  owned.push(() => context.dispose());
  return {
    auth,
    channel,
    context,
    subscribers,
    change(patch: Partial<typeof state>, nextRevision = revision) {
      state = { ...state, ...patch };
      revision = nextRevision;
      for (const listener of subscribers) listener();
    },
  };
}

test("stable auth notifications preserve a token, account/revision/working transitions never restore an old one", () => {
  const s = fixture();
  const first = s.context.capture();
  expect(first).not.toBeNull();
  s.change({});
  expect(s.context.capture()).toBe(first);
  s.change({
    account: { id: "a0000000-0000-4000-8000-000000000001", nickname: "player" },
  });
  const account = s.context.capture();
  expect(account).not.toBe(first);
  s.change({}, 2);
  expect(s.context.capture()).not.toBe(account);
  s.change({ working: true });
  expect(s.context.capture()).toBeNull();
  s.change({ working: false, account: null });
  expect(s.context.capture()).not.toBe(first);
});

test("peer signals only invalidate, payloads cannot grant or leak and receiving never rebroadcasts", () => {
  const s = fixture();
  const listener = vi.fn();
  s.context.subscribe(listener);
  const first = s.context.capture();
  s.channel.dispatchEvent(
    new MessageEvent("message", {
      data: { status: "valid", analytics: true, token: "private" },
    }),
  );
  expect(s.context.capture()).toBe(first);
  s.channel.dispatchEvent(new MessageEvent("message", { data: "invalidate" }));
  expect(s.context.capture()).not.toBe(first);
  expect(listener).toHaveBeenCalledOnce();
  expect(s.channel.postMessage).not.toHaveBeenCalled();
  s.context.broadcast();
  expect(s.channel.postMessage.mock.calls).toEqual([["invalidate"]]);
});

test("offline, hidden and restored pages invalidate before reuse and remove observers on dispose", () => {
  const s = fixture();
  const first = s.context.capture();
  const online = vi
    .spyOn(window.navigator, "onLine", "get")
    .mockReturnValue(false);
  window.dispatchEvent(new Event("offline"));
  expect(s.context.capture()).toBeNull();
  online.mockReturnValue(true);
  window.dispatchEvent(new Event("online"));
  expect(s.context.capture()).not.toBe(first);
  const visible = vi
    .spyOn(document, "visibilityState", "get")
    .mockReturnValue("hidden");
  document.dispatchEvent(new Event("visibilitychange"));
  expect(s.context.capture()).toBeNull();
  visible.mockReturnValue("visible");
  document.dispatchEvent(new Event("visibilitychange"));
  const beforeRestore = s.context.capture();
  window.dispatchEvent(
    new PageTransitionEvent("pageshow", { persisted: true }),
  );
  expect(s.context.capture()).not.toBe(beforeRestore);
  s.context.dispose();
  expect(s.subscribers.size).toBe(0);
  expect(s.channel.close).toHaveBeenCalledOnce();
  expect(s.context.capture()).toBeNull();
});

test("channel absence, messaging failure and messageerror block optional choices", () => {
  const s = fixture();
  const missing = createConsentContext(window, s.auth, null);
  owned.push(() => missing.dispose());
  expect(missing.capture()).toBeNull();
  s.channel.postMessage.mockImplementation(() => {
    throw Error("private");
  });
  s.context.broadcast();
  expect(s.context.capture()).toBeNull();
  const other = fixture();
  other.channel.dispatchEvent(new MessageEvent("messageerror"));
  expect(other.context.capture()).toBeNull();
});

test("unconfigured composition creates no account observer, channel or successful reopen", async () => {
  const s = fixture();
  s.context.dispose();
  const consent = createBrowserConsent(window, s.auth);
  owned.push(() => consent.dispose());
  expect(s.subscribers.size).toBe(0);
  expect(consent.read()).toMatchObject({
    status: "unavailable",
    analytics: false,
    advertising: false,
    canReopen: false,
  });
  expect(await consent.reopen()).toBe(false);
});

test("browser composition consumes only captured receipts and broadcasts an invalidation on choice change", async () => {
  const s = fixture();
  s.context.dispose();
  let value: unknown;
  let changed = () => {};
  const adapter: ConsentAdapter = {
    read: () => value,
    subscribe(listener) {
      changed = listener;
      return vi.fn();
    },
    reopen: async () => {},
    dispose: vi.fn(),
  };
  const factory: ConsentAdapterFactory = (context) => {
    value = {
      status: "valid",
      context: context.capture(),
      revision: 1,
      analytics: true,
      advertising: false,
      validFrom: Date.now() - 10,
      expiresAt: Date.now() + 60000,
    };
    return adapter;
  };
  const channel = Object.assign(new EventTarget(), {
    postMessage: vi.fn(),
    close: vi.fn(),
  });
  const consent = createBrowserConsent(window, s.auth, factory, channel);
  owned.push(() => consent.dispose());
  expect(consent.read().analytics).toBe(true);
  value = { status: "denied" };
  changed();
  expect(consent.read().analytics).toBe(false);
  expect(channel.postMessage.mock.calls).toEqual([["invalidate"]]);
  consent.dispose();
  consent.dispose();
  expect(adapter.dispose).toHaveBeenCalledOnce();
  expect(channel.close).toHaveBeenCalledOnce();
});
