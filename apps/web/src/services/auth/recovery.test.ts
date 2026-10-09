// @vitest-environment node
import { afterEach, expect, test, vi } from "vitest";
import type { AuthBootstrap } from "@liar/protocol";
import { createAuth } from "./session";
import { createDailyRecords } from "../daily-records";
import { createPendingSubmissions } from "../pending-submissions";
import { record } from "../../../test/daily-record";
import {
  AuthConnectionError,
  AuthError,
  PROVIDERS,
  type AuthTransport,
} from "./types";

const account = {
  id: "a0000000-0000-4000-8000-000000000001",
  nickname: "player",
};
const session = "b0000000-0000-4000-8000-000000000001";
const snapshot = (): AuthBootstrap => ({
  account,
  session_revision: session,
  csrf: "private-memory-proof",
  providers: PROVIDERS.map((p) => ({ provider: p.id, available: true })),
});
async function fixture() {
  let online = true;
  const transport: AuthTransport = {
    bootstrap: vi.fn().mockResolvedValue(snapshot()),
    identities: vi
      .fn()
      .mockResolvedValue([{ provider: "google", linked_at: 1 }]),
    start: vi.fn(),
    nickname: vi.fn(),
    export: vi.fn(),
    erase: vi.fn(),
    unlink: vi.fn(),
    logout: vi.fn(),
  };
  const auth = createAuth(
    transport,
    () => online,
    () => {},
    () => Date.now(),
  );
  await auth.refresh();
  vi.mocked(transport.bootstrap).mockClear();
  return {
    auth,
    transport,
    owner: { accountId: account.id, revision: auth.revision() },
    offline: () => {
      online = false;
      auth.suspend();
    },
    online: () => {
      online = true;
    },
  };
}
afterEach(() => vi.useRealTimers());

function deferred<T>() {
  let resolve!: (value: T) => void;
  return {
    promise: new Promise<T>((r) => {
      resolve = r;
    }),
    resolve: (value: T) => resolve(value),
  };
}

test("a submission interrupted by suspension retains its record even after the same identity recovers before a late accepted reply", async () => {
  const s = await fixture(),
    records = createDailyRecords(),
    reply = deferred<"accepted">();
  await records.saveFirst(record());
  const submit = vi.fn(() => reply.promise);
  const queue = createPendingSubmissions(records, s.auth, { submit });
  try {
    const pending = queue.flush(account.id);
    await vi.waitFor(() => expect(submit).toHaveBeenCalledTimes(1));
    s.offline();
    s.online();
    expect((await s.auth.resume(s.owner)).ok).toBe(true);
    expect(s.auth.revision()).toBe(s.owner.revision);
    reply.resolve("accepted");
    expect((await pending).retained).toBe(true);
    expect((await records.pending()).value).toEqual([record()]);
  } finally {
    s.auth.dispose();
  }
});

test("a pre-suspension authenticated response stays cancelled after recovery even though the owner revision is identical", async () => {
  const s = await fixture(),
    late = deferred<string>();
  const target = vi.fn(() => late.promise);
  try {
    const old = s.auth.execute(s.owner, target);
    const cancelled = expect(old).rejects.toMatchObject({ code: "stale" });
    await vi.waitFor(() => expect(target).toHaveBeenCalledTimes(1));
    s.offline();
    s.online();
    expect((await s.auth.resume(s.owner)).ok).toBe(true);
    expect(s.auth.revision()).toBe(s.owner.revision);
    await cancelled;
    expect(await s.auth.execute(s.owner, async () => "fresh")).toBe("fresh");
    late.resolve("old");
    await Promise.resolve();
    await Promise.resolve();
    expect(s.auth.read().account).toEqual(account);
  } finally {
    s.auth.dispose();
  }
});

test("classified proof transport failures suspend authority while arbitrary exceptions and malformed proofs do not create a candidate", async () => {
  for (const error of [
    new AuthConnectionError(),
    new Error("unknown"),
    new AuthError("auth_unavailable"),
  ]) {
    const s = await fixture();
    try {
      vi.mocked(s.transport.bootstrap).mockRejectedValueOnce(error);
      await expect(
        s.auth.execute(s.owner, async () => "unsafe"),
      ).rejects.toMatchObject({ code: "auth_unavailable" });
      expect(s.auth.account()).toBeNull();
      expect(s.auth.connected()).toBe(false);
      expect(s.auth.recoveryOwner()).toEqual(
        error instanceof AuthConnectionError ? s.owner : null,
      );
    } finally {
      s.auth.dispose();
    }
  }
});

test("resume uses the final accepted nickname and retries a transport outage without resetting the original budget", async () => {
  vi.useFakeTimers();
  const s = await fixture();
  try {
    s.offline();
    s.online();
    vi.mocked(s.transport.bootstrap).mockRejectedValueOnce(
      new AuthConnectionError(),
    );
    expect((await s.auth.resume(s.owner)).ok).toBe(false);
    expect(s.auth.recoveryOwner()).toEqual(s.owner);
    expect(s.auth.account()).toBeNull();
    await vi.advanceTimersByTimeAsync(29000);
    vi.mocked(s.transport.bootstrap)
      .mockResolvedValueOnce(snapshot())
      .mockResolvedValueOnce({
        ...snapshot(),
        account: { ...account, nickname: "accepted" },
      });
    expect((await s.auth.resume(s.owner)).ok).toBe(true);
    expect(s.auth.read().account?.nickname).toBe("accepted");
    await vi.advanceTimersByTimeAsync(1000);
    expect(s.auth.connected()).toBe(true);
  } finally {
    s.auth.dispose();
  }
});

test("one stalled resume settles at 10 seconds and its ignored late response cannot affect a successful retry", async () => {
  vi.useFakeTimers();
  const s = await fixture(),
    late = deferred<AuthBootstrap>();
  try {
    s.offline();
    s.online();
    vi.mocked(s.transport.bootstrap).mockImplementationOnce(() => late.promise);
    const first = s.auth.resume(s.owner);
    expect((await s.auth.resume(s.owner)).ok).toBe(false);
    expect(s.transport.bootstrap).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(9999);
    expect(s.auth.account()).toBeNull();
    await vi.advanceTimersByTimeAsync(1);
    expect(await first).toEqual({ ok: false, code: "auth_unavailable" });
    expect(s.auth.recoveryOwner()).toEqual(s.owner);
    expect(s.transport.bootstrap).toHaveBeenCalledTimes(1);
    expect((await s.auth.resume(s.owner)).ok).toBe(true);
    const reads = vi.mocked(s.transport.identities).mock.calls.length;
    late.resolve({ ...snapshot(), session_revision: "replacement" });
    await Promise.resolve();
    await Promise.resolve();
    expect(s.transport.identities).toHaveBeenCalledTimes(reads);
    expect(s.auth.read().account).toEqual(account);
    expect(s.auth.revision()).toBe(s.owner.revision);
  } finally {
    s.auth.dispose();
  }
});

test("the total budget cuts off a stalled last proof and never revives the owner after its exact deadline", async () => {
  vi.useFakeTimers();
  const s = await fixture(),
    late = deferred<AuthBootstrap>();
  try {
    s.offline();
    await vi.advanceTimersByTimeAsync(29000);
    s.online();
    vi.mocked(s.transport.bootstrap)
      .mockResolvedValueOnce(snapshot())
      .mockImplementationOnce(() => late.promise);
    const pending = s.auth.resume(s.owner);
    await vi.advanceTimersByTimeAsync(999);
    expect(s.auth.recoveryOwner()).toEqual(s.owner);
    await vi.advanceTimersByTimeAsync(1);
    expect((await pending).ok).toBe(false);
    expect(s.auth.recoveryOwner()).toBeNull();
    late.resolve(snapshot());
    await Promise.resolve();
    await Promise.resolve();
    expect(s.auth.account()).toBeNull();
    expect(s.auth.revision()).toBeGreaterThan(s.owner.revision);
  } finally {
    s.auth.dispose();
  }
});

test.each(["invalidate", "refresh", "write", "dispose"])(
  "%s discards the candidate and makes an overlapping ignored proof harmless",
  async (change) => {
    const s = await fixture(),
      late = deferred<AuthBootstrap>();
    s.offline();
    s.online();
    vi.mocked(s.transport.bootstrap).mockImplementationOnce(() => late.promise);
    const pending = s.auth.resume(s.owner);
    if (change === "invalidate") s.auth.invalidate();
    else if (change === "write")
      expect((await s.auth.erase(account.id)).ok).toBe(false);
    else if (change === "dispose") s.auth.dispose();
    else {
      vi.mocked(s.transport.bootstrap).mockResolvedValue({
        ...snapshot(),
        account: { ...account, id: "a0000000-0000-4000-8000-000000000002" },
      });
      await s.auth.refresh();
    }
    expect((await pending).ok).toBe(false);
    expect(s.auth.recoveryOwner()).toBeNull();
    const state = s.auth.read();
    late.resolve(snapshot());
    await Promise.resolve();
    await Promise.resolve();
    expect(s.auth.read()).toEqual(state);
    expect(s.transport.erase).not.toHaveBeenCalled();
    s.auth.dispose();
  },
);

test("guests and unfinished nicknames never acquire recovery identity", async () => {
  for (const value of [null, { ...account, nickname: null }]) {
    const s = await fixture();
    vi.mocked(s.transport.bootstrap).mockResolvedValue({
      ...snapshot(),
      account: value,
      session_revision: value ? session : null,
    });
    await s.auth.refresh();
    s.offline();
    expect(s.auth.recoveryOwner()).toBeNull();
    s.auth.dispose();
  }
});

test("a suspended owner is not authority and is restored only after fresh proof of the same session", async () => {
  const s = await fixture();
  try {
    s.offline();
    expect(s.auth.account()).toBeNull();
    expect(s.auth.connected()).toBe(false);
    expect(s.auth.recoveryOwner()).toEqual(s.owner);
    expect(s.auth.revision()).toBe(s.owner.revision);
    const work = vi.fn();
    await expect(s.auth.execute(s.owner, work)).rejects.toMatchObject({
      code: "unavailable",
    });
    expect(work).not.toHaveBeenCalled();
    expect(await s.auth.resume(s.owner)).toEqual({
      ok: false,
      code: "auth_unavailable",
    });
    expect(s.transport.bootstrap).not.toHaveBeenCalled();
    expect(
      JSON.stringify({
        state: s.auth.read(),
        recovery: s.auth.recoveryOwner(),
      }),
    ).not.toMatch(/csrf|session|token|credential|nickname|private-memory/);
    s.online();
    expect(await s.auth.resume(s.owner)).toEqual({
      ok: true,
      value: undefined,
    });
    expect(s.transport.bootstrap).toHaveBeenCalledTimes(2);
    expect(s.auth.read().account).toEqual(account);
    expect(s.auth.recoveryOwner()).toBeNull();
    expect(s.auth.revision()).toBe(s.owner.revision);
    expect(s.auth.connected()).toBe(true);
    expect(await s.auth.execute(s.owner, async () => "accepted")).toBe(
      "accepted",
    );
  } finally {
    s.auth.dispose();
  }
});

test.each(["account", "session", "last-proof", "malformed", "unauthorized"])(
  "recovery rejects %s and never adopts an effective replacement authority",
  async (change) => {
    const s = await fixture();
    try {
      s.offline();
      s.online();
      const changed = {
        ...snapshot(),
        ...(change === "account"
          ? {
              account: {
                ...account,
                id: "a0000000-0000-4000-8000-000000000002",
              },
            }
          : { session_revision: "b0000000-0000-4000-8000-000000000002" }),
      };
      const boot = vi.mocked(s.transport.bootstrap);
      if (change === "last-proof")
        boot.mockResolvedValueOnce(snapshot()).mockResolvedValueOnce(changed);
      else if (change === "malformed" || change === "unauthorized")
        boot.mockRejectedValueOnce(
          new AuthError(
            change === "malformed" ? "auth_unavailable" : "auth_required",
          ),
        );
      else boot.mockResolvedValueOnce(changed);
      expect((await s.auth.resume(s.owner)).ok).toBe(false);
      expect(s.auth.account()).toBeNull();
      expect(s.auth.recoveryOwner()).toBeNull();
      expect(s.auth.revision()).toBeGreaterThan(s.owner.revision);
    } finally {
      s.auth.dispose();
    }
  },
);

test("the first interruption's exact 30 second budget is not extended by repeated offline events", async () => {
  vi.useFakeTimers();
  const s = await fixture();
  try {
    s.offline();
    await vi.advanceTimersByTimeAsync(20000);
    s.offline();
    await vi.advanceTimersByTimeAsync(9999);
    expect(s.auth.recoveryOwner()).toEqual(s.owner);
    await vi.advanceTimersByTimeAsync(1);
    expect(s.auth.recoveryOwner()).toBeNull();
    expect(s.auth.revision()).toBeGreaterThan(s.owner.revision);
    s.online();
    expect((await s.auth.resume(s.owner)).ok).toBe(false);
    expect(s.transport.bootstrap).not.toHaveBeenCalled();
  } finally {
    s.auth.dispose();
  }
});
