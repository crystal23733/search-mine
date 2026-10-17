// @vitest-environment node
import { afterEach, expect, test, vi } from "vitest";
import { createConsent, type CmpPort } from "./consent";
import { createConsentGuard, type ConsentContextPort } from "./consent-guard";

const owned: Array<() => void> = [];
afterEach(() => {
  for (const dispose of owned.splice(0)) dispose();
  vi.useRealTimers();
});
function fixture() {
  vi.useFakeTimers();
  let token: object = Object.freeze({});
  let ready = true;
  const subscribers = new Set<() => void>();
  const context: ConsentContextPort = {
    capture: () => (ready ? token : null),
    subscribe: (listener) => {
      subscribers.add(listener);
      return () => {
        subscribers.delete(listener);
      };
    },
    invalidate() {
      token = Object.freeze({});
      for (const listener of subscribers) listener();
    },
  };
  const time = { wall: 1000, monotonic: 10 };
  let value: unknown;
  let changed = () => {};
  const cmp: CmpPort = {
    read: vi.fn(() => value),
    subscribe: vi.fn((listener) => {
      changed = listener;
      return vi.fn();
    }),
    reopen: vi.fn(async () => {}),
  };
  const choice = (patch: Record<string, unknown> = {}) => ({
    status: "valid",
    analytics: true,
    advertising: false,
    context: token,
    revision: 1,
    validFrom: 1000,
    expiresAt: 2000,
    ...patch,
  });
  value = choice();
  const guard = createConsentGuard(cmp, context, () => ({ ...time }));
  const consent = createConsent(guard);
  const dispose = () => {
    consent.dispose();
    guard.dispose();
  };
  owned.push(dispose);
  return {
    context,
    time,
    cmp,
    guard,
    consent,
    choice,
    dispose,
    subscribers,
    set(next: unknown) {
      value = next;
      changed();
    },
    ready(next: boolean) {
      ready = next;
      context.invalidate();
    },
    advance(ms: number) {
      time.wall += ms;
      time.monotonic += ms;
      vi.advanceTimersByTime(ms);
    },
  };
}

test("current verified receipt authorizes only its explicit purpose and leaks no metadata", () => {
  const s = fixture();
  expect(s.consent.read()).toMatchObject({
    status: "valid",
    analytics: true,
    advertising: false,
  });
  expect(Object.keys(s.consent.read()).sort()).toEqual([
    "advertising",
    "analytics",
    "canReopen",
    "status",
    "working",
  ]);
  s.set(s.choice({ revision: 2, analytics: false, advertising: true }));
  expect(s.consent.read()).toMatchObject({
    analytics: false,
    advertising: true,
  });
});

test.each([
  { expiresAt: 1000 },
  { validFrom: 1001 },
  { expiresAt: 999 },
  { revision: 0 },
  { revision: 1.1 },
  { revision: Number.MAX_SAFE_INTEGER + 1 },
  { validFrom: -1 },
  { validFrom: NaN },
  { expiresAt: Infinity },
  { analytics: "true" },
  { advertising: 1 },
  { token: "private" },
  { context: {} },
  { context: null },
])(
  "invalid, expired, future or foreign receipt blocks purposes: %j",
  (patch) => {
    const s = fixture();
    s.set(s.choice(patch));
    expect(s.consent.read()).toMatchObject({
      analytics: false,
      advertising: false,
    });
    expect(JSON.stringify(s.consent.read())).not.toContain("private");
  },
);

test("expiry notifies subscribers, retires the revision and never extends from repeated reads", () => {
  const s = fixture();
  const listener = vi.fn();
  s.consent.subscribe(listener);
  expect(s.consent.read().analytics).toBe(true);
  expect(vi.getTimerCount()).toBe(1);
  s.advance(999);
  expect(s.consent.read().analytics).toBe(true);
  s.advance(1);
  expect(s.consent.read()).toMatchObject({
    status: "unknown",
    analytics: false,
  });
  expect(listener).toHaveBeenCalled();
  expect(vi.getTimerCount()).toBe(0);
  s.time.wall = 1500;
  expect(s.consent.read().analytics).toBe(false);
});

test("lower or modified same revisions cannot restore a choice; a fresh higher revision can", () => {
  const s = fixture();
  expect(s.consent.read().analytics).toBe(true);
  s.set(s.choice({ analytics: false, advertising: true }));
  expect(s.consent.read().advertising).toBe(false);
  s.set(s.choice());
  expect(s.consent.read().analytics).toBe(false);
  s.set(s.choice({ revision: 3 }));
  expect(s.consent.read().analytics).toBe(true);
  s.set(s.choice({ revision: 2 }));
  expect(s.consent.read().analytics).toBe(false);
  s.set(s.choice({ revision: 4, analytics: false, advertising: true }));
  expect(s.consent.read().advertising).toBe(true);
});

test.each(["unknown", "denied", "error"])(
  "%s retires an old revision and requires fresh verification",
  (status) => {
    const s = fixture();
    expect(s.consent.read().analytics).toBe(true);
    s.set({ status });
    expect(s.consent.read().analytics).toBe(false);
    s.set(s.choice());
    expect(s.consent.read().analytics).toBe(false);
    s.set(s.choice({ revision: 2 }));
    expect(s.consent.read().analytics).toBe(true);
  },
);

test("context change aborts pending reopen and rejects a late receipt from its old context", async () => {
  const s = fixture();
  const old = s.choice();
  let signal!: AbortSignal;
  let finish!: () => void;
  vi.mocked(s.cmp.reopen).mockImplementation((input) => {
    signal = input;
    return new Promise<void>((resolve) => {
      finish = resolve;
    });
  });
  const reopening = s.consent.reopen();
  await vi.waitFor(() => expect(s.cmp.reopen).toHaveBeenCalledOnce());
  s.context.invalidate();
  expect(signal.aborted).toBe(true);
  expect(await reopening).toBe(false);
  s.set(old);
  finish();
  await Promise.resolve();
  expect(s.consent.read()).toMatchObject({
    analytics: false,
    advertising: false,
    working: false,
  });
  s.set(s.choice({ revision: 2 }));
  expect(s.consent.read().analytics).toBe(true);
});

test.each(["wall", "monotonic"] as const)(
  "%s clock regression blocks until a new context verifies a choice",
  (field) => {
    const s = fixture();
    expect(s.consent.read().analytics).toBe(true);
    s.time[field]--;
    expect(s.consent.read()).toMatchObject({
      status: "error",
      analytics: false,
    });
    s.time[field] += 2;
    expect(s.consent.read().analytics).toBe(false);
    s.context.invalidate();
    s.set(s.choice());
    expect(s.consent.read().analytics).toBe(true);
  },
);

test("monotonic deadline expires even when the wall clock advances too slowly", () => {
  const s = fixture();
  expect(s.consent.read().analytics).toBe(true);
  s.time.wall += 10;
  s.time.monotonic += 1000;
  expect(s.consent.read().analytics).toBe(false);
});

test("unknown context, read failure and disposal block with subscriptions and timers released", () => {
  const s = fixture();
  expect(s.consent.read().analytics).toBe(true);
  s.ready(false);
  expect(s.consent.read().analytics).toBe(false);
  s.ready(true);
  vi.mocked(s.cmp.read).mockImplementation(() => {
    throw Error("private");
  });
  expect(s.consent.read()).toMatchObject({ status: "error", analytics: false });
  s.dispose();
  expect(s.subscribers.size).toBe(0);
  expect(vi.getTimerCount()).toBe(0);
  expect(s.guard.read()).toEqual({ status: "error" });
});

test("long expiry schedules bounded chunks rather than overflowing the browser timer", () => {
  const s = fixture();
  s.set(s.choice({ expiresAt: 1000 + 2147483647 + 500 }));
  expect(s.consent.read().analytics).toBe(true);
  s.advance(2147483647);
  expect(s.consent.read().analytics).toBe(true);
  expect(vi.getTimerCount()).toBe(1);
  s.advance(500);
  expect(s.consent.read().analytics).toBe(false);
});

test("failed adapter subscription releases an already attached context observer", () => {
  const listeners = new Set<() => void>();
  const context: ConsentContextPort = {
    capture: () => ({}),
    invalidate() {},
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
  const guard = createConsentGuard(
    {
      read: () => ({}),
      reopen: async () => {},
      subscribe() {
        throw Error("private");
      },
    },
    context,
  );
  owned.push(() => guard.dispose());
  expect(guard.read()).toEqual({ status: "error" });
  expect(listeners.size).toBe(0);
});

test.each(["abort", "dispose"])(
  "%s releases a caller even if the adapter ignores cancellation",
  async (action) => {
    const s = fixture();
    let signal!: AbortSignal;
    vi.mocked(s.cmp.reopen).mockImplementation((input) => {
      signal = input;
      return new Promise(() => {});
    });
    const controller = new AbortController();
    const result = s.consent.reopen(controller.signal);
    await vi.waitFor(() => expect(s.cmp.reopen).toHaveBeenCalledOnce());
    if (action === "abort") controller.abort();
    else s.dispose();
    expect(signal.aborted).toBe(true);
    expect(await result).toBe(false);
  },
);

test("a context read error also cancels a pending adapter without a notification", async () => {
  const s = fixture();
  let signal!: AbortSignal;
  vi.mocked(s.cmp.reopen).mockImplementation((input) => {
    signal = input;
    return new Promise(() => {});
  });
  const result = s.consent.reopen();
  await vi.waitFor(() => expect(s.cmp.reopen).toHaveBeenCalledOnce());
  vi.spyOn(s.context, "capture").mockImplementation(() => {
    throw Error("private");
  });
  expect(s.guard.read()).toEqual({ status: "error" });
  expect(signal.aborted).toBe(true);
  expect(await result).toBe(false);
});

test.each(["wall", "monotonic"] as const)(
  "invalid %s clock values fail closed",
  (field) => {
    const s = fixture();
    expect(s.consent.read().analytics).toBe(true);
    s.time[field] = NaN;
    expect(s.consent.read()).toMatchObject({
      status: "error",
      analytics: false,
    });
  },
);

test("same revision cannot extend expiry and a changed context cancels its timer", () => {
  const s = fixture();
  expect(s.consent.read().analytics).toBe(true);
  s.set(s.choice({ expiresAt: 3000 }));
  expect(s.consent.read().analytics).toBe(false);
  expect(vi.getTimerCount()).toBe(0);
  s.set(s.choice({ revision: 2 }));
  expect(s.consent.read().analytics).toBe(true);
  expect(vi.getTimerCount()).toBe(1);
  s.context.invalidate();
  expect(vi.getTimerCount()).toBe(0);
  expect(s.consent.read().analytics).toBe(false);
});

test.each([null, [], { status: "valid" }, { status: "unknown", extra: true }])(
  "malformed source state fails closed: %j",
  (value) => {
    const s = fixture();
    s.set(value);
    expect(s.consent.read()).toMatchObject({
      status: "error",
      analytics: false,
      advertising: false,
    });
  },
);

test("unavailable context and pre-aborted direct requests never open the adapter", async () => {
  const s = fixture();
  s.ready(false);
  expect(await s.consent.reopen()).toBe(false);
  const signal = AbortSignal.abort();
  await expect(s.guard.reopen(signal)).rejects.toThrow();
  expect(s.cmp.reopen).not.toHaveBeenCalled();
});

test("adapter failure keeps choices blocked and a context changed without a callback rejects completion", async () => {
  const s = fixture();
  vi.mocked(s.cmp.reopen).mockRejectedValueOnce(Error("private"));
  expect(await s.consent.reopen()).toBe(false);
  expect(s.consent.read()).toMatchObject({ status: "error", analytics: false });
  let finish!: () => void;
  vi.mocked(s.cmp.reopen).mockImplementation(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  const result = s.consent.reopen();
  await vi.waitFor(() => expect(s.cmp.reopen).toHaveBeenCalledTimes(2));
  vi.spyOn(s.context, "capture").mockReturnValue(Object.freeze({}));
  finish();
  expect(await result).toBe(false);
});

test("failed synchronous setup cleans a timer and observer; invalid removers fail closed", () => {
  const s = fixture();
  const token = s.context.capture();
  s.set(s.choice({ context: token }));
  vi.mocked(s.cmp.subscribe).mockImplementation((listener) => {
    listener();
    throw Error("private");
  });
  const guard = createConsentGuard(s.cmp, s.context, () => ({ ...s.time }));
  owned.push(() => guard.dispose());
  expect(() => guard.subscribe(() => {})).toThrow();
  expect(guard.read()).toEqual({ status: "error" });
  expect(vi.getTimerCount()).toBe(1);
  s.dispose();
  expect(vi.getTimerCount()).toBe(0);
  const invalid = createConsentGuard(
    { ...s.cmp, subscribe: () => null as unknown as () => void },
    s.context,
  );
  owned.push(() => invalid.dispose());
  expect(invalid.read()).toEqual({ status: "error" });
});

test.each(["read", "property"])(
  "a context change during adapter %s cannot return an old grant",
  (stage) => {
    const s = fixture();
    const old = s.choice();
    if (stage === "property")
      Object.defineProperty(old, "analytics", {
        enumerable: true,
        get() {
          s.context.invalidate();
          return true;
        },
      });
    vi.mocked(s.cmp.read).mockImplementationOnce(() => {
      if (stage === "read") s.context.invalidate();
      return old;
    });
    expect(s.consent.read()).toMatchObject({
      analytics: false,
      advertising: false,
    });
    expect(vi.getTimerCount()).toBe(0);
  },
);

test("a receipt that expires while the adapter is reading is blocked on that same read", () => {
  const s = fixture();
  const old = s.choice();
  vi.mocked(s.cmp.read).mockImplementationOnce(() => {
    s.time.wall = 2000;
    s.time.monotonic = 1010;
    return old;
  });
  expect(s.consent.read().analytics).toBe(false);
  expect(vi.getTimerCount()).toBe(0);
});
