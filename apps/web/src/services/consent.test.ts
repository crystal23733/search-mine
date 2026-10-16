// @vitest-environment node
import { expect, test, vi } from "vitest";
import { createConsent, type CmpPort } from "./consent";

test("valid analytics choice does not authorize advertising and every read checks current choice", () => {
  let choice: unknown = {
    status: "valid",
    analytics: true,
    advertising: false,
  };
  const cmp: CmpPort = {
    read: () => choice,
    subscribe: () => () => {},
    reopen: vi.fn(async () => {}),
  };
  const consent = createConsent(cmp);
  expect(consent.read()).toMatchObject({
    status: "valid",
    analytics: true,
    advertising: false,
  });
  choice = { status: "denied" };
  expect(consent.read()).toMatchObject({
    status: "denied",
    analytics: false,
    advertising: false,
  });
  choice = { status: "valid", analytics: false, advertising: true };
  expect(consent.read()).toMatchObject({ analytics: false, advertising: true });
  expect(cmp.reopen).not.toHaveBeenCalled();
  consent.dispose();
});

function fixture() {
  let choice: unknown = { status: "valid", analytics: true, advertising: true };
  let listener = () => {};
  const unsubscribe = vi.fn();
  const cmp: CmpPort = {
    read: vi.fn(() => choice),
    subscribe: vi.fn((changed) => {
      listener = changed;
      return unsubscribe;
    }),
    reopen: vi.fn(async () => {}),
  };
  const consent = createConsent(cmp);
  return {
    cmp,
    consent,
    unsubscribe,
    change: (next: unknown) => {
      choice = next;
      listener();
    },
  };
}

test.each([
  null,
  [],
  { status: "unknown" },
  { status: "denied" },
  { status: "error" },
  { status: "valid", analytics: "true", advertising: true },
  { status: "valid", analytics: true },
  { status: "valid", analytics: true, advertising: true, token: "private" },
  { status: "other", analytics: true, advertising: true },
])(
  "missing, denied, unknown or malformed choice never grants a purpose: %j",
  (choice) => {
    const s = fixture();
    s.change(choice);
    expect(s.consent.read()).toMatchObject({
      analytics: false,
      advertising: false,
      working: false,
    });
    expect(JSON.stringify(s.consent.read())).not.toContain("private");
    s.consent.dispose();
  },
);

test("unconfigured CMP never opens and adapter read/subscription errors fail closed", async () => {
  const missing = createConsent();
  expect(missing.read()).toMatchObject({
    status: "unavailable",
    canReopen: false,
  });
  expect(await missing.reopen()).toBe(false);
  missing.dispose();
  const s = fixture();
  vi.mocked(s.cmp.read).mockImplementation(() => {
    throw Error("private");
  });
  expect(s.consent.read()).toMatchObject({
    status: "error",
    analytics: false,
    advertising: false,
  });
  s.consent.dispose();
  vi.mocked(s.cmp.subscribe).mockImplementation(() => {
    throw Error("private");
  });
  const broken = createConsent(s.cmp);
  expect(broken.read()).toMatchObject({ status: "error", canReopen: false });
  expect(await broken.reopen()).toBe(false);
  broken.dispose();
});

test("revocation notifies subscribers, every read uses current state, and unsubscription detaches", () => {
  const s = fixture();
  const listener = vi.fn();
  const remove = s.consent.subscribe(listener);
  const old = s.consent.read();
  s.change({ status: "denied" });
  expect(listener).toHaveBeenCalledOnce();
  expect(old.analytics).toBe(true);
  expect(s.consent.read()).toMatchObject({
    status: "denied",
    analytics: false,
    advertising: false,
  });
  remove();
  s.change({ status: "unknown" });
  expect(listener).toHaveBeenCalledOnce();
  s.consent.dispose();
  s.consent.dispose();
  expect(s.unsubscribe).toHaveBeenCalledOnce();
});

test("one pending reopen blocks both purposes; completion reads revocation instead of an old grant", async () => {
  const s = fixture();
  let finish!: () => void;
  vi.mocked(s.cmp.reopen).mockImplementation(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  const opening = s.consent.reopen();
  await Promise.resolve();
  expect(s.consent.read()).toMatchObject({
    working: true,
    analytics: false,
    advertising: false,
  });
  expect(await s.consent.reopen()).toBe(false);
  expect(s.cmp.reopen).toHaveBeenCalledOnce();
  s.change({ status: "denied" });
  finish();
  expect(await opening).toBe(true);
  expect(s.consent.read()).toMatchObject({
    status: "denied",
    working: false,
    analytics: false,
    advertising: false,
  });
  s.consent.dispose();
});

test("synchronous or asynchronous reopen failure blocks a previously valid choice until a current change", async () => {
  const s = fixture();
  vi.mocked(s.cmp.reopen).mockImplementationOnce(() => {
    throw Error("private");
  });
  expect(await s.consent.reopen()).toBe(false);
  expect(s.consent.read()).toMatchObject({
    status: "error",
    analytics: false,
    advertising: false,
  });
  s.change({ status: "valid", analytics: true, advertising: false });
  expect(s.consent.read().analytics).toBe(true);
  vi.mocked(s.cmp.reopen).mockRejectedValueOnce(Error("private"));
  expect(await s.consent.reopen()).toBe(false);
  expect(s.consent.read()).toMatchObject({
    status: "error",
    analytics: false,
    advertising: false,
  });
  s.consent.dispose();
});

test("abort resolves immediately even when CMP ignores it; a late completion cannot finish a newer reopen", async () => {
  const s = fixture();
  const pending: Array<() => void> = [];
  const signals: AbortSignal[] = [];
  vi.mocked(s.cmp.reopen).mockImplementation((signal) => {
    signals.push(signal);
    return new Promise<void>((resolve) => {
      pending.push(resolve);
    });
  });
  const caller = new AbortController();
  const first = s.consent.reopen(caller.signal);
  await Promise.resolve();
  caller.abort();
  expect(await first).toBe(false);
  expect(signals[0].aborted).toBe(true);
  const second = s.consent.reopen();
  await Promise.resolve();
  pending[0]();
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  expect(s.consent.read().working).toBe(true);
  s.change({ status: "denied" });
  pending[1]();
  expect(await second).toBe(true);
  expect(s.consent.read().advertising).toBe(false);
  s.consent.dispose();
});

test("pre-aborted calls do not open; dispose cancels work and ignores future CMP callbacks", async () => {
  const s = fixture();
  const caller = new AbortController();
  caller.abort();
  expect(await s.consent.reopen(caller.signal)).toBe(false);
  expect(s.cmp.reopen).not.toHaveBeenCalled();
  vi.mocked(s.cmp.reopen).mockImplementation(() => new Promise<void>(() => {}));
  const opening = s.consent.reopen();
  await Promise.resolve();
  const listener = vi.fn();
  s.consent.subscribe(listener);
  s.consent.dispose();
  expect(await opening).toBe(false);
  s.change({ status: "valid", analytics: true, advertising: true });
  expect(listener).not.toHaveBeenCalled();
  expect(s.consent.read()).toMatchObject({
    status: "unavailable",
    canReopen: false,
    working: false,
    analytics: false,
    advertising: false,
  });
  expect(s.unsubscribe).toHaveBeenCalledOnce();
});

test("an invalid subscription cleanup keeps the gate closed and cannot break disposal", () => {
  const s = fixture();
  s.consent.dispose();
  vi.mocked(s.cmp.subscribe).mockReturnValueOnce(
    undefined as unknown as () => void,
  );
  const broken = createConsent(s.cmp);
  expect(broken.read()).toMatchObject({
    status: "error",
    analytics: false,
    advertising: false,
    canReopen: false,
  });
  expect(() => broken.dispose()).not.toThrow();
  const listener = vi.fn();
  broken.subscribe(listener);
  s.change({ status: "valid", analytics: true, advertising: true });
  expect(listener).not.toHaveBeenCalled();
});

test("cancellation during synchronous publication prevents the adapter from opening", async () => {
  const s = fixture(),
    caller = new AbortController();
  s.consent.subscribe(() => {
    if (s.consent.read().working) caller.abort();
  });
  expect(await s.consent.reopen(caller.signal)).toBe(false);
  expect(s.cmp.reopen).not.toHaveBeenCalled();
  s.consent.dispose();
});
