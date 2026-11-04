// @vitest-environment node
import { expect, test, vi } from "vitest";
import { authTestPorts, TEST_ACCOUNT } from "../../../test/auth-ports";
import { createConsentHttp, createEventHttp } from "./http";
import type {
  ConsentSnapshot,
  OptionalEventCaptureInput,
} from "@liar/protocol";
const revision = "c0000000-0000-4000-8000-000000000001";
export function snapshot(): ConsentSnapshot {
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
async function fixture(raw: unknown = snapshot(), status = 200) {
  const f = authTestPorts({ ...TEST_ACCOUNT, nickname: null }, true);
  await f.auth.refresh();
  const fetcher = vi.fn<typeof fetch>().mockResolvedValue(
    new Response(JSON.stringify(raw), {
      status,
      headers: { "content-type": "application/json" },
    }),
  );
  const mutation = vi.fn();
  return {
    ...f,
    fetcher,
    mutation,
    api: createConsentHttp(f.auth, fetcher, mutation),
    owner: { accountId: TEST_ACCOUNT.id, revision: f.auth.revision() },
    signal: new AbortController().signal,
  };
}
test("consent reads use self authority before nickname setup and closed current snapshots", async () => {
  const f = await fixture();
  expect(await f.api.read(f.owner, f.signal)).toEqual(snapshot());
  expect(f.fetcher.mock.calls[0]).toMatchObject([
    "/api/v1/me/consent",
    {
      method: "POST",
      credentials: "same-origin",
      cache: "no-store",
      redirect: "error",
      body: JSON.stringify({ v: 1 }),
      headers: {
        "content-type": "application/json",
        "x-liar-csrf": "fixture-memory-csrf",
      },
    },
  ]);
  expect(f.mutation).not.toHaveBeenCalled();
});
test("withdrawal invalidates optional work before writing and accepts only the denied current revision", async () => {
  const value = snapshot();
  value.consent.analytics = "denied";
  const f = await fixture(value);
  expect(await f.api.withdraw(f.owner, "all", f.signal)).toEqual(value);
  expect(f.mutation).toHaveBeenCalledOnce();
  expect(f.fetcher.mock.calls[0][1]?.body).toBe(
    JSON.stringify({ v: 1, purpose: "all" }),
  );
});
test("foreign accounts, secret fields, inconsistent revisions and times cannot become authority", async () => {
  for (const mutate of [
    (v: ConsentSnapshot) => ({ ...v, account_id: revision }),
    (v: ConsentSnapshot) => ({ ...v, secret: "forbidden" }),
    (v: ConsentSnapshot) => ({ ...v, revision: null }),
    (v: ConsentSnapshot) => ({ ...v, observed_at: 999 }),
  ]) {
    const f = await fixture(mutate(snapshot()));
    await expect(f.api.read(f.owner, f.signal)).rejects.toBeTruthy();
  }
});
test("conflicts and expired authority do not retry or mint a local grant", async () => {
  const f = await fixture({ error: "conflict" }, 409);
  await expect(
    f.api.refresh(f.owner, revision, f.signal),
  ).rejects.toMatchObject({ code: "conflict" });
  expect(f.fetcher).toHaveBeenCalledTimes(1);
});
test("event receipts match the single immutable ID and contain no extra fields", async () => {
  const input: OptionalEventCaptureInput = {
    v: 1,
    expected_revision: revision,
    draft: {
      event_version: 1,
      event_id: revision,
      session_key: TEST_ACCOUNT.id,
      occurred_at: 1500,
      locale: "ko",
      event: { type: "landing_view", route: "home" },
    },
  };
  const f = await fixture({ v: 1, event_id: revision, received_at: 1500 });
  expect(
    await createEventHttp(f.auth, f.fetcher).send(f.owner, input, f.signal),
  ).toEqual({ v: 1, event_id: revision, received_at: 1500 });
  expect(f.fetcher.mock.calls[0][0]).toBe("/api/v1/events/optional");
});

test("a withdrawal acknowledgment that preserves the withdrawn grant is rejected", async () => {
  const f = await fixture();
  await expect(
    f.api.withdraw(f.owner, "analytics", f.signal),
  ).rejects.toMatchObject({ code: "unavailable" });
});
test("successful refresh must acknowledge a new current server revision", async () => {
  const f = await fixture();
  await expect(f.api.refresh(f.owner, revision, f.signal)).rejects.toBeTruthy();
  const g = await fixture();
  expect((await g.api.refresh(g.owner, null, g.signal)).revision).toBe(
    revision,
  );
});
test("an unauthorized response invalidates self authority and a secret receipt is rejected", async () => {
  const f = await fixture({ error: "unauthorized" }, 401);
  await expect(f.api.read(f.owner, f.signal)).rejects.toBeTruthy();
  expect(f.auth.read().account).toBeNull();
  const g = await fixture({
    v: 1,
    event_id: revision,
    received_at: 1500,
    account_id: TEST_ACCOUNT.id,
  });
  await expect(
    createEventHttp(g.auth, g.fetcher).send(
      g.owner,
      {
        v: 1,
        expected_revision: revision,
        draft: {
          event_version: 1,
          event_id: revision,
          session_key: TEST_ACCOUNT.id,
          occurred_at: 1500,
          locale: "ko",
          event: { type: "invite_open" },
        },
      },
      g.signal,
    ),
  ).rejects.toBeTruthy();
});
