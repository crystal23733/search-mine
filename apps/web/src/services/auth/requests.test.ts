// @vitest-environment node
import { afterEach, expect, test, vi } from "vitest";
import type { AuthBootstrap } from "@liar/protocol";
import { createAuth } from "./session";
import { AuthError, PROVIDERS, type AuthTransport } from "./types";
import { AuthenticatedRequestError, type RequestContext } from "./requests";

const a = { id: "a0000000-0000-4000-8000-000000000001", nickname: "player" };
const b = { id: "a0000000-0000-4000-8000-000000000002", nickname: "other" };
const sessionA = "b0000000-0000-4000-8000-000000000001";
const sessionB = "b0000000-0000-4000-8000-000000000002";
function snapshot(
  account: AuthBootstrap["account"] = a,
  session = sessionA,
  csrf = "fresh-memory-proof",
): AuthBootstrap {
  return {
    account,
    session_revision: account ? session : null,
    csrf,
    providers: PROVIDERS.map((p) => ({ provider: p.id, available: true })),
  };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((r, j) => {
    resolve = r;
    reject = j;
  });
  return { promise, resolve, reject };
}
async function fixture(online: () => boolean = () => true) {
  const t: AuthTransport = {
    bootstrap: vi.fn().mockResolvedValue(snapshot()),
    identities: vi.fn().mockResolvedValue([]),
    start: vi.fn(),
    nickname: vi.fn().mockResolvedValue(a),
    export: vi.fn(),
    erase: vi.fn(),
    unlink: vi.fn(),
    logout: vi.fn(),
  };
  const auth = createAuth(t, online);
  await auth.refresh();
  vi.mocked(t.bootstrap).mockClear();
  return { t, auth, owner: { accountId: a.id, revision: auth.revision() } };
}
afterEach(() => vi.useRealTimers());

test("parallel requests refresh proof and verify replies without changing stable auth state", async () => {
  const { t, auth, owner } = await fixture();
  const state = auth.read(),
    changed = vi.fn(),
    unsubscribe = auth.subscribe(changed),
    one = deferred<string>(),
    two = deferred<string>();
  const targetA = vi.fn(({ csrf }: RequestContext) => {
    expect(csrf).toBe("request-one-proof");
    return one.promise;
  });
  const targetB = vi.fn(({ csrf }: RequestContext) => {
    expect(csrf).toBe("request-two-proof");
    return two.promise;
  });
  vi.mocked(t.bootstrap)
    .mockResolvedValueOnce(snapshot(a, sessionA, "request-one-proof"))
    .mockResolvedValueOnce(snapshot(a, sessionA, "request-two-proof"));
  const first = auth.execute(owner, targetA),
    second = auth.execute(owner, targetB);
  await vi.waitFor(() => expect(targetB).toHaveBeenCalledTimes(1));
  two.resolve("second");
  expect(await second).toBe("second");
  expect(targetA.mock.calls[0]![0].signal.aborted).toBe(false);
  one.resolve("first");
  expect(await first).toBe("first");
  expect(t.bootstrap).toHaveBeenCalledTimes(4);
  expect(auth.read()).toEqual(state);
  expect(auth.revision()).toBe(owner.revision);
  expect(changed).not.toHaveBeenCalled();
  expect(JSON.stringify(auth.read())).not.toMatch(
    /csrf|session|token|credential/,
  );
  unsubscribe();
  auth.dispose();
});

test("unavailable, stale and pre-cancelled owners never reach bootstrap or target", async () => {
  let online = true;
  const { t, auth, owner } = await fixture(() => online);
  const target = vi.fn().mockResolvedValue("unsafe");
  await expect(
    auth.execute({ ...owner, accountId: b.id }, target),
  ).rejects.toMatchObject({ code: "stale" });
  await expect(
    auth.execute({ ...owner, revision: owner.revision - 1 }, target),
  ).rejects.toMatchObject({ code: "stale" });
  const cancelled = new AbortController();
  cancelled.abort();
  await expect(
    auth.execute(owner, target, cancelled.signal),
  ).rejects.toMatchObject({ code: "cancelled" });
  online = false;
  await expect(auth.execute(owner, target)).rejects.toMatchObject({
    code: "unavailable",
  });
  online = true;
  expect(t.bootstrap).not.toHaveBeenCalled();
  vi.mocked(t.bootstrap).mockResolvedValue(snapshot({ ...a, nickname: null }));
  await auth.refresh();
  vi.mocked(t.bootstrap).mockClear();
  await expect(
    auth.execute({ ...owner, revision: auth.revision() }, target),
  ).rejects.toMatchObject({ code: "unavailable" });
  vi.mocked(t.bootstrap).mockResolvedValue(snapshot(null));
  await auth.refresh();
  vi.mocked(t.bootstrap).mockClear();
  await expect(auth.execute(owner, target)).rejects.toMatchObject({
    code: "unavailable",
  });
  auth.dispose();
  await expect(auth.execute(owner, target)).rejects.toMatchObject({
    code: "unavailable",
  });
  expect(target).not.toHaveBeenCalled();
  expect(t.bootstrap).not.toHaveBeenCalled();
});

test.each([
  ["account", snapshot(b, sessionB)],
  ["same account rotated session", snapshot(a, sessionB)],
  ["logged out", snapshot(null)],
])(
  "pre-request proof rejects %s and invalidates only that authority",
  async (_, fresh) => {
    const { t, auth, owner } = await fixture();
    vi.mocked(t.bootstrap).mockResolvedValue(fresh);
    const target = vi.fn().mockResolvedValue("unsafe");
    await expect(auth.execute(owner, target)).rejects.toMatchObject({
      code: "auth_invalid",
    });
    expect(target).not.toHaveBeenCalled();
    expect(auth.account()).toBeNull();
    expect(auth.connected()).toBe(false);
    expect(auth.revision()).toBeGreaterThan(owner.revision);
    auth.dispose();
  },
);

test("a reply is not exposed when the effective cookie session changes during the target", async () => {
  const { t, auth, owner } = await fixture();
  const target = vi.fn(async () => {
    vi.mocked(t.bootstrap).mockResolvedValue(snapshot(a, sessionB));
    return "old-session-result";
  });
  await expect(auth.execute(owner, target)).rejects.toMatchObject({
    code: "auth_invalid",
  });
  expect(target).toHaveBeenCalledTimes(1);
  expect(t.bootstrap).toHaveBeenCalledTimes(2);
  expect(auth.connected()).toBe(false);
  auth.dispose();
});

test("domain errors keep their identity and do not revoke or rewrite auth state", async () => {
  const { t, auth, owner } = await fixture();
  const state = auth.read(),
    domain = { code: "room_full", detail: "fixture" };
  await expect(
    auth.execute(owner, async () => {
      throw domain;
    }),
  ).rejects.toBe(domain);
  expect(t.bootstrap).toHaveBeenCalledTimes(1);
  expect(auth.read()).toEqual(state);
  expect(auth.revision()).toBe(owner.revision);
  await expect(
    auth.execute(owner, async () => {
      throw new AuthError("auth_required");
    }),
  ).rejects.toMatchObject({ code: "auth_required" });
  expect(auth.connected()).toBe(false);
  auth.dispose();
});

test("proof outages fail closed while an older failure cannot clear a refreshed account", async () => {
  const { t, auth, owner } = await fixture();
  vi.mocked(t.bootstrap).mockRejectedValueOnce(new Error("network"));
  await expect(auth.execute(owner, async () => "unsafe")).rejects.toMatchObject(
    { code: "auth_unavailable" },
  );
  expect(auth.account()).toBeNull();
  await auth.refresh();
  const oldProof = deferred<AuthBootstrap>();
  vi.mocked(t.bootstrap).mockImplementationOnce(() => oldProof.promise);
  const target = vi.fn().mockResolvedValue("unsafe"),
    old = auth.execute({ ...owner, revision: auth.revision() }, target);
  const rejected = expect(old).rejects.toMatchObject({ code: "stale" });
  vi.mocked(t.bootstrap).mockResolvedValue(snapshot(b, sessionB));
  await auth.refresh();
  oldProof.reject(new AuthError("auth_required"));
  await rejected;
  expect(auth.read().account).toEqual(b);
  expect(auth.connected()).toBe(true);
  expect(target).not.toHaveBeenCalled();
  auth.dispose();
});

test("caller cancellation settles promptly, leaves peers intact and blocks ignored late bootstrap", async () => {
  const { t, auth, owner } = await fixture();
  const late = deferred<AuthBootstrap>(),
    caller = new AbortController(),
    target = vi.fn().mockResolvedValue("unsafe");
  vi.mocked(t.bootstrap).mockImplementationOnce(() => late.promise);
  const pending = auth.execute(owner, target, caller.signal),
    rejected = expect(pending).rejects.toMatchObject({ code: "cancelled" });
  caller.abort();
  await rejected;
  expect(await auth.execute(owner, async () => "peer")).toBe("peer");
  late.resolve(snapshot());
  await Promise.resolve();
  await Promise.resolve();
  expect(target).not.toHaveBeenCalled();
  expect(auth.connected()).toBe(true);
  expect(auth.revision()).toBe(owner.revision);
  auth.dispose();
});

test.each(["refresh", "invalidate", "nickname", "dispose"] as const)(
  "%s aborts in-flight work and rejects ignored late target replies",
  async (action) => {
    const { auth, owner } = await fixture();
    const late = deferred<string>(),
      target = vi.fn(() => late.promise),
      pending = auth.execute(owner, target),
      rejected = expect(pending).rejects.toMatchObject({ code: "stale" });
    await vi.waitFor(() => expect(target).toHaveBeenCalledTimes(1));
    if (action === "nickname") await auth.nickname("player", a.id);
    else await auth[action]();
    await rejected;
    late.resolve("old-target-result");
    await Promise.resolve();
    expect(auth.read().working).toBe(false);
    auth.dispose();
  },
);

test("the ninth active request is rejected and a cancelled slot can be reused", async () => {
  const { t, auth, owner } = await fixture();
  const late = deferred<string>(),
    callers = Array.from({ length: 8 }, () => new AbortController()),
    target = vi.fn(() => late.promise),
    pending = callers.map((c) => auth.execute(owner, target, c.signal)),
    rejected = pending.map((p) =>
      expect(p).rejects.toBeInstanceOf(AuthenticatedRequestError),
    );
  await vi.waitFor(() => expect(target).toHaveBeenCalledTimes(8));
  await expect(auth.execute(owner, target)).rejects.toMatchObject({
    code: "capacity",
  });
  expect(t.bootstrap).toHaveBeenCalledTimes(8);
  callers[0]!.abort();
  await rejected[0];
  expect(await auth.execute(owner, async () => "reused")).toBe("reused");
  for (const caller of callers) caller.abort();
  await Promise.all(rejected);
  expect(auth.connected()).toBe(true);
  auth.dispose();
});

test("the ten second budget includes pre-proof, target and post-proof without automatic retry", async () => {
  const { t, auth, owner } = await fixture();
  vi.useFakeTimers();
  const late = deferred<AuthBootstrap>(),
    target = vi.fn().mockResolvedValue("private-result");
  vi.mocked(t.bootstrap)
    .mockImplementationOnce(async () => {
      await new Promise((r) => setTimeout(r, 4000));
      return snapshot();
    })
    .mockImplementationOnce(() => late.promise);
  const pending = auth.execute(owner, target),
    rejected = expect(pending).rejects.toMatchObject({ code: "timeout" });
  await vi.advanceTimersByTimeAsync(9999);
  expect(target).toHaveBeenCalledTimes(1);
  expect(t.bootstrap).toHaveBeenCalledTimes(2);
  expect(target.mock.calls[0]![0].signal.aborted).toBe(false);
  await vi.advanceTimersByTimeAsync(1);
  await rejected;
  expect(target.mock.calls[0]![0].signal.aborted).toBe(true);
  expect(auth.connected()).toBe(true);
  expect(auth.revision()).toBe(owner.revision);
  late.resolve(snapshot(b, sessionB));
  await vi.advanceTimersByTimeAsync(0);
  expect(auth.read().account).toEqual(a);
  expect(vi.getTimerCount()).toBe(0);
  auth.dispose();
});

test("an old target authentication failure cannot revoke a newer account", async () => {
  const { t, auth, owner } = await fixture();
  const late = deferred<string>(),
    target = vi.fn(() => late.promise),
    old = auth.execute(owner, target),
    rejected = expect(old).rejects.toMatchObject({ code: "stale" });
  await vi.waitFor(() => expect(target).toHaveBeenCalledTimes(1));
  vi.mocked(t.bootstrap).mockResolvedValue(snapshot(b, sessionB));
  await auth.refresh();
  await rejected;
  late.reject(new AuthError("auth_required"));
  await Promise.resolve();
  await Promise.resolve();
  expect(auth.read().account).toEqual(b);
  expect(auth.connected()).toBe(true);
  auth.dispose();
});

test("cancelling a target removes its listener and does not cancel a concurrent peer", async () => {
  const { auth, owner } = await fixture();
  const caller = new AbortController(),
    remove = vi.spyOn(caller.signal, "removeEventListener"),
    late = deferred<string>(),
    peer = deferred<string>(),
    target = vi.fn((_context: RequestContext) => late.promise),
    other = vi.fn((_context: RequestContext) => peer.promise),
    pending = auth.execute(owner, target, caller.signal),
    rejected = expect(pending).rejects.toMatchObject({ code: "cancelled" }),
    activePeer = auth.execute(owner, other);
  await vi.waitFor(() => expect(other).toHaveBeenCalledTimes(1));
  caller.abort();
  await rejected;
  expect(target.mock.calls[0]![0].signal.aborted).toBe(true);
  expect(other.mock.calls[0]![0].signal.aborted).toBe(false);
  expect(remove).toHaveBeenCalledWith("abort", expect.any(Function));
  late.resolve("ignored");
  peer.resolve("accepted-peer");
  expect(await activePeer).toBe("accepted-peer");
  expect(auth.revision()).toBe(owner.revision);
  auth.dispose();
});

test("timeout before proof returns never starts a target even if the transport ignores abort", async () => {
  const { t, auth, owner } = await fixture();
  vi.useFakeTimers();
  const late = deferred<AuthBootstrap>(),
    target = vi.fn().mockResolvedValue("unsafe");
  vi.mocked(t.bootstrap).mockImplementationOnce(() => late.promise);
  const pending = auth.execute(owner, target),
    rejected = expect(pending).rejects.toMatchObject({ code: "timeout" });
  await vi.advanceTimersByTimeAsync(10_000);
  await rejected;
  late.resolve(snapshot());
  await vi.advanceTimersByTimeAsync(0);
  expect(target).not.toHaveBeenCalled();
  expect(t.bootstrap).toHaveBeenCalledTimes(1);
  expect(vi.getTimerCount()).toBe(0);
  expect(await auth.execute(owner, async () => "fresh-request")).toBe(
    "fresh-request",
  );
  expect(vi.getTimerCount()).toBe(0);
  auth.dispose();
});
