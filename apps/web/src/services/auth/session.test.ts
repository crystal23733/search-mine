// @vitest-environment node
import { expect, test, vi } from "vitest";
import { createAuth } from "./session";
import { PROVIDERS, AuthError, type AuthTransport } from "./types";
import { createDailyRecords } from "../daily-records";
import { createPendingSubmissions } from "../pending-submissions";
import { record } from "../../../test/daily-record";
test("writes refresh memory CSRF and reconcile the effective session before any mutation", async () => {
  const t = transport(),
    auth = createAuth(t);
  await auth.refresh();
  vi.mocked(t.bootstrap).mockResolvedValue({
    ...snapshot(),
    csrf: "fresh-memory-proof",
  });
  await auth.nickname("newname", a.id);
  expect(t.nickname).toHaveBeenCalledWith(
    "newname",
    "fresh-memory-proof",
    expect.any(AbortSignal),
  );
  vi.mocked(t.bootstrap).mockResolvedValue(
    snapshot(b, "b0000000-0000-4000-8000-000000000002"),
  );
  expect(await auth.erase(a.id)).toEqual({ ok: false, code: "auth_invalid" });
  expect(t.erase).not.toHaveBeenCalled();
  expect(auth.connected()).toBe(false);
});

test("normalization comes from the nickname response and unauthorized writes revoke memory authority", async () => {
  const t = transport(),
    auth = createAuth(t);
  await auth.refresh();
  expect(await auth.nickname("e\u0301探偵", a.id)).toEqual({
    ok: true,
    value: { ...a, nickname: "é探偵" },
  });
  expect(auth.read().account?.nickname).toBe("é探偵");
  vi.mocked(t.export).mockRejectedValue(new AuthError("auth_required"));
  expect(await auth.export(a.id)).toEqual({ ok: false, code: "auth_required" });
  expect(auth.account()).toBeNull();
  expect(auth.connected()).toBe(false);
});
test("an old ownership-mismatched export reply cannot clear a newer account", async () => {
  const t = transport(),
    auth = createAuth(t);
  await auth.refresh();
  let reply!: (value: Awaited<ReturnType<AuthTransport["export"]>>) => void;
  vi.mocked(t.export).mockImplementationOnce(
    () =>
      new Promise((r) => {
        reply = r;
      }),
  );
  const old = auth.export(a.id);
  await vi.waitFor(() => expect(t.export).toHaveBeenCalledTimes(1));
  vi.mocked(t.bootstrap).mockResolvedValue(
    snapshot(b, "b0000000-0000-4000-8000-000000000002"),
  );
  await auth.refresh();
  reply({ account: b, created_at: 1, last_seen_at: 2, identities: [] });
  expect(await old).toEqual({ ok: false, code: "auth_invalid" });
  expect(auth.read().account).toEqual(b);
  expect(auth.connected()).toBe(true);
});

const a = { id: "a0000000-0000-4000-8000-000000000001", nickname: "player" };
const b = { id: "a0000000-0000-4000-8000-000000000002", nickname: "other" };
const snapshot = (
  account: typeof a | null = a,
  revision = "b0000000-0000-4000-8000-000000000001",
) => ({
  account,
  session_revision: account ? revision : null,
  csrf: "fixture-memory-csrf",
  providers: PROVIDERS.map((p) => ({ provider: p.id, available: true })),
});
function transport(): AuthTransport {
  return {
    bootstrap: vi.fn().mockResolvedValue(snapshot()),
    identities: vi
      .fn()
      .mockResolvedValue([{ provider: "google", linked_at: 1 }]),
    start: vi.fn().mockResolvedValue({
      authorize_url: "https://accounts.google.com/o/oauth2/v2/auth",
    }),
    nickname: vi.fn().mockResolvedValue({ ...a, nickname: "é探偵" }),
    export: vi.fn().mockResolvedValue({
      account: a,
      created_at: 1,
      last_seen_at: 2,
      identities: [{ provider: "google", linked_at: 1 }],
    }),
    erase: vi.fn().mockResolvedValue({ manual_apple_disconnect: true }),
    unlink: vi.fn().mockResolvedValue({ manual_apple_disconnect: false }),
    logout: vi.fn().mockResolvedValue(undefined),
  };
}
test("bootstrap uses authoritative account and session while exposing no CSRF or credentials", async () => {
  const auth = createAuth(transport());
  await auth.refresh();
  expect(auth.read()).toMatchObject({
    status: "ready",
    account: a,
    identities: [{ provider: "google", linked_at: 1 }],
  });
  expect(auth.account()).toBe(a.id);
  expect(auth.connected()).toBe(true);
  expect(JSON.stringify(auth.read())).not.toMatch(
    /csrf|credential|subject|token|password/,
  );
  const old = auth.revision();
  auth.invalidate();
  expect(auth.account()).toBeNull();
  expect(auth.connected()).toBe(false);
  expect(auth.revision()).toBeGreaterThan(old);
});
test("overlapping bootstrap completions and identity reads cannot resurrect the previous account", async () => {
  const t = transport();
  let reply!: (value: ReturnType<typeof snapshot>) => void;
  vi.mocked(t.bootstrap)
    .mockReset()
    .mockImplementationOnce(
      () =>
        new Promise((r) => {
          reply = r;
        }),
    )
    .mockResolvedValue(snapshot(b, "b0000000-0000-4000-8000-000000000002"));
  const auth = createAuth(t);
  const first = auth.refresh();
  expect(t.bootstrap).toHaveBeenCalledTimes(1);
  const second = auth.refresh();
  await second;
  reply(snapshot(a));
  await first;
  expect(auth.read().account).toEqual(b);
  vi.mocked(t.bootstrap)
    .mockReset()
    .mockResolvedValueOnce(snapshot(a))
    .mockResolvedValue(snapshot(b, "b0000000-0000-4000-8000-000000000002"));
  await auth.refresh();
  expect(auth.account()).not.toBe(a.id);
});
test("a destructive confirmation bound to A cannot mutate B and reauthentication is never an automatic delete", async () => {
  const t = transport(),
    auth = createAuth(t);
  await auth.refresh();
  vi.mocked(t.bootstrap).mockResolvedValue(
    snapshot(b, "b0000000-0000-4000-8000-000000000002"),
  );
  await auth.refresh();
  expect(await auth.erase(a.id)).toEqual({ ok: false, code: "auth_invalid" });
  expect(t.erase).not.toHaveBeenCalled();
  vi.mocked(t.erase).mockRejectedValue(new AuthError("reauth_required"));
  expect(await auth.erase(b.id)).toEqual({
    ok: false,
    code: "reauth_required",
  });
  expect(t.erase).toHaveBeenCalledTimes(1);
  expect(t.start).not.toHaveBeenCalled();
});

test("completion observers cannot apply an old nickname or erasure notice after starting a new authority generation", async () => {
  const t = transport(),
    auth = createAuth(t);
  await auth.refresh();
  let armed = false;
  const unsubscribe = auth.subscribe(() => {
    const state = auth.read();
    if (state.working) armed = true;
    else if (armed && state.status === "ready") {
      armed = false;
      auth.invalidate();
    }
  });
  expect((await auth.nickname("old-result", a.id)).ok).toBe(false);
  expect(auth.account()).toBeNull();
  unsubscribe();
});

test("a deletion acknowledgement cannot label a newer account as deleted", async () => {
  const t = transport(),
    auth = createAuth(t);
  await auth.refresh();
  let resolve!: (value: ReturnType<typeof snapshot>) => void;
  vi.mocked(t.bootstrap)
    .mockImplementationOnce(async () => snapshot())
    .mockImplementationOnce(
      () =>
        new Promise((r) => {
          resolve = r;
        }),
    );
  const erasure = auth.erase(a.id);
  await vi.waitFor(() => expect(resolve).toBeTypeOf("function"));
  vi.mocked(t.bootstrap).mockResolvedValue(
    snapshot(b, "b0000000-0000-4000-8000-000000000002"),
  );
  await auth.refresh();
  resolve(snapshot(null));
  await erasure;
  expect(auth.read()).toMatchObject({ account: b, notice: null });
});

test("a later authority refresh clears a completed action's account-specific notice", async () => {
  const t = transport(),
    auth = createAuth(t);
  await auth.refresh();
  vi.mocked(t.erase).mockImplementationOnce(async () => {
    vi.mocked(t.bootstrap).mockResolvedValue(snapshot(null));
    return { manual_apple_disconnect: true };
  });
  await auth.erase(a.id);
  expect(auth.read()).toMatchObject({
    notice: "deleted",
    manualAppleDisconnect: true,
  });
  vi.mocked(t.bootstrap).mockResolvedValue(snapshot(b));
  await auth.refresh();
  expect(auth.read()).toMatchObject({
    account: b,
    notice: null,
    manualAppleDisconnect: false,
  });
});

test("the actual auth revision retains a pending result after logout, outage and a late acknowledgement", async () => {
  const t = transport(),
    auth = createAuth(t),
    records = createDailyRecords();
  await auth.refresh();
  await records.saveFirst(record());
  let reply!: (value: "accepted") => void;
  const submit = vi.fn(
    () =>
      new Promise<"accepted">((resolve) => {
        reply = resolve;
      }),
  );
  const queue = createPendingSubmissions(records, auth, { submit });
  const pending = queue.flush(a.id);
  await vi.waitFor(() => expect(submit).toHaveBeenCalledTimes(1));
  vi.mocked(t.bootstrap).mockResolvedValue(snapshot(null));
  await auth.logout();
  reply("accepted");
  expect((await pending).retained).toBe(true);
  auth.invalidate();
  await queue.flush(a.id);
  expect(submit).toHaveBeenCalledTimes(1);
  expect((await records.pending()).value).toEqual([record()]);
});
