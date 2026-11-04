// @vitest-environment node
import { afterEach, expect, test, vi } from "vitest";
import type {
  ConsentSnapshot,
  OptionalEventCaptureInput,
} from "@liar/protocol";
import { authTestPorts, TEST_ACCOUNT } from "../../../test/auth-ports";
import { createConsent } from "../consent";
import { createOptionalEvents, type CollectorDependencies } from "./collector";
const revision = "c0000000-0000-4000-8000-000000000001";
function snapshot(): ConsentSnapshot {
  return {
    v: 1,
    account_id: TEST_ACCOUNT.id,
    observed_at: 1500,
    revision,
    consent: {
      cmp_available: false,
      status: "current",
      analytics: "granted",
      advertising: "denied",
      checked_at: 1000,
      expires_at: 2000,
    },
  };
}
async function fixture() {
  const f = authTestPorts({ ...TEST_ACCOUNT, nickname: null }, true);
  await f.auth.refresh();
  const listeners = new Set<() => void>();
  let choice = { status: "valid", analytics: true, advertising: false };
  let online = true;
  let time = { wall: 1500000, monotonic: 100 };
  let id = 0;
  const consent = createConsent({
    read: () => choice,
    subscribe: (fn) => {
      listeners.add(fn);
      return () => {
        listeners.delete(fn);
      };
    },
    reopen: async () => {},
  });
  const control = {
    read: vi.fn().mockResolvedValue(snapshot()),
    refresh: vi.fn(),
    withdraw: vi.fn(),
  };
  const transport = {
    send: vi
      .fn()
      .mockImplementation(async (_owner, input: OptionalEventCaptureInput) => ({
        v: 1,
        event_id: input.draft.event_id,
        received_at: 1500,
      })),
  };
  const deps: CollectorDependencies = {
    auth: f.auth,
    consent,
    control,
    transport,
    available: () => online,
    clock: () => ({ ...time }),
    id: vi.fn(
      () => `d0000000-0000-4000-8000-${String(++id).padStart(12, "0")}`,
    ),
  };
  const collector = createOptionalEvents(deps);
  return {
    ...f,
    consent,
    control,
    transport,
    collector,
    deps,
    setChoice: (v: typeof choice) => {
      choice = v;
      for (const fn of listeners) fn();
    },
    offline: () => {
      online = false;
    },
    setTime: (v: typeof time) => {
      time = v;
    },
  };
}
const landing = { type: "landing_view", route: "home" } as const;
afterEach(() => vi.useRealTimers());
test("both current purposes and a server revision are required before a single minimal event is sent", async () => {
  const f = await fixture();
  expect(await f.collector.record(landing, "ko")).toBe(true);
  expect(f.control.read).toHaveBeenCalledOnce();
  expect(f.transport.send).toHaveBeenCalledOnce();
  const [owner, input] = f.transport.send.mock.calls[0];
  expect(owner.accountId).toBe(TEST_ACCOUNT.id);
  expect(input).toMatchObject({
    v: 1,
    expected_revision: revision,
    draft: {
      event_version: 1,
      occurred_at: 1500,
      locale: "ko",
      event: landing,
    },
  });
  expect(Object.keys(input).sort()).toEqual([
    "draft",
    "expected_revision",
    "v",
  ]);
  expect(input.draft.event_id).not.toBe(input.draft.session_key);
  expect(input.draft.session_key).not.toBe(TEST_ACCOUNT.id);
  f.collector.dispose();
});
test("unconfigured CMP, denied purpose, guest or offline creates no optional request or correlation ID", async () => {
  for (const mode of ["missing", "advertising", "guest", "offline"]) {
    const f = await fixture();
    if (mode === "missing")
      f.setChoice({ status: "unknown", analytics: false, advertising: false });
    if (mode === "guest") await f.auth.logout();
    if (mode === "offline") f.offline();
    expect(
      await f.collector.record(
        mode === "advertising"
          ? { type: "ad_request", placement: "menu" }
          : landing,
        "ko",
      ),
    ).toBe(false);
    expect(f.control.read).not.toHaveBeenCalled();
    expect(f.transport.send).not.toHaveBeenCalled();
    expect(f.deps.id).not.toHaveBeenCalled();
    f.collector.dispose();
  }
});
test("server denial, foreign owner and expired snapshots are never replaced with a local grant", async () => {
  for (const mode of ["denied", "foreign", "expired"]) {
    const f = await fixture();
    const value = snapshot();
    if (mode === "denied") value.consent.analytics = "denied";
    if (mode === "foreign") value.account_id = revision;
    if (mode === "expired") {
      value.consent.status = "expired";
      value.consent.expires_at = 1500;
    }
    f.control.read.mockResolvedValue(value);
    expect(await f.collector.record(landing, "en")).toBe(false);
    expect(f.transport.send).not.toHaveBeenCalled();
    f.collector.dispose();
  }
});
test("pending records are dropped and cancellation discards ignored late server work without retry", async () => {
  const f = await fixture();
  let finish!: (value: ConsentSnapshot) => void;
  f.control.read.mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const pending = f.collector.record(landing, "en");
  await Promise.resolve();
  expect(await f.collector.record(landing, "en")).toBe(false);
  f.collector.cancel();
  expect(await pending).toBe(false);
  finish(snapshot());
  await Promise.resolve();
  expect(f.transport.send).not.toHaveBeenCalled();
  f.collector.dispose();
});
test("a revoked browser purpose or changed authentication cancels in-flight transmission", async () => {
  for (const mode of ["consent", "auth"]) {
    const f = await fixture();
    let finish!: () => void;
    f.transport.send.mockImplementation(
      (_o, input) =>
        new Promise((resolve) => {
          finish = () =>
            resolve({
              v: 1,
              event_id: input.draft.event_id,
              received_at: 1500,
            });
        }),
    );
    const pending = f.collector.record(landing, "ko");
    await vi.waitFor(() => expect(f.transport.send).toHaveBeenCalledOnce());
    const signal = f.transport.send.mock.calls[0][2] as AbortSignal;
    if (mode === "consent")
      f.setChoice({ status: "denied", analytics: false, advertising: false });
    else f.auth.invalidate();
    expect(signal.aborted).toBe(true);
    expect(await pending).toBe(false);
    finish();
    f.collector.dispose();
  }
});
test("the whole optional operation ends at two seconds even when an adapter ignores abort", async () => {
  vi.useFakeTimers();
  const f = await fixture();
  f.control.read.mockImplementation(() => new Promise(() => {}));
  const pending = f.collector.record(landing, "ko");
  await vi.advanceTimersByTimeAsync(2000);
  expect(await pending).toBe(false);
  expect(f.transport.send).not.toHaveBeenCalled();
  f.collector.dispose();
});
test("wall rollback and time spent reading server permission cannot extend a grant", async () => {
  for (const rollback of [true, false]) {
    const f = await fixture();
    const value = snapshot();
    value.consent.expires_at = 1501;
    f.control.read.mockImplementation(async () => {
      f.setTime(
        rollback
          ? { wall: 1499000, monotonic: 101 }
          : { wall: 1501000, monotonic: 1100 },
      );
      return value;
    });
    expect(await f.collector.record(landing, "ko")).toBe(false);
    expect(f.transport.send).not.toHaveBeenCalled();
    f.collector.dispose();
  }
});
test("tab keys live only within the same owner and reset after cancellation", async () => {
  const f = await fixture();
  await f.collector.record(landing, "en");
  await f.collector.record(landing, "en");
  const a = f.transport.send.mock.calls[0][1].draft,
    b = f.transport.send.mock.calls[1][1].draft;
  expect(a.session_key).toBe(b.session_key);
  expect(a.event_id).not.toBe(b.event_id);
  f.collector.cancel();
  await f.collector.record(landing, "en");
  expect(f.transport.send.mock.calls[2][1].draft.session_key).not.toBe(
    a.session_key,
  );
  f.collector.dispose();
});

test("a new page can record immediately after cancellation without old cleanup erasing its tab key", async () => {
  const f = await fixture();
  let finish!: (v: ConsentSnapshot) => void;
  f.control.read.mockImplementationOnce(
    () =>
      new Promise((r) => {
        finish = r;
      }),
  );
  const old = f.collector.record(landing, "en");
  await Promise.resolve();
  f.collector.cancel();
  expect(await f.collector.record(landing, "ko")).toBe(true);
  expect(await old).toBe(false);
  finish(snapshot());
  await Promise.resolve();
  const key = f.transport.send.mock.calls[0][1].draft.session_key;
  expect(await f.collector.record(landing, "ko")).toBe(true);
  expect(f.transport.send.mock.calls[1][1].draft.session_key).toBe(key);
  f.collector.dispose();
});

test("advertising permission is independent of analytics and still needs its current server grant", async () => {
  const f = await fixture();
  f.setChoice({ status: "valid", analytics: false, advertising: true });
  const value = snapshot();
  value.consent.analytics = "denied";
  value.consent.advertising = "granted";
  f.control.read.mockResolvedValue(value);
  expect(
    await f.collector.record(
      { type: "ad_impression", placement: "result" },
      "ko",
    ),
  ).toBe(true);
  expect(await f.collector.record(landing, "ko")).toBe(false);
  expect(f.transport.send).toHaveBeenCalledTimes(1);
  f.collector.dispose();
});
