// @vitest-environment node
import { expect, test, vi } from "vitest";
import { createMeHttp } from "./http";
import { authTestPorts, TEST_ACCOUNT } from "../../../test/auth-ports";
import { overview, history } from "../../../test/me-data";
import { MeFailure } from "./types";
async function fixture(
  value: unknown = overview(),
  nickname: string | null = null,
) {
  const ports = authTestPorts({ ...TEST_ACCOUNT, nickname }, true);
  await ports.auth.refresh();
  const fetcher = vi.fn<typeof fetch>().mockImplementation(
    async () =>
      new Response(JSON.stringify(value), {
        headers: { "content-type": "application/json" },
      }),
  );
  return {
    ...ports,
    fetcher,
    api: createMeHttp(ports.auth, fetcher),
    owner: { accountId: TEST_ACCOUNT.id, revision: ports.auth.revision() },
    signal: new AbortController().signal,
  };
}
test("ordinary me reads use current self proofs, closed bodies and no-store without a nickname", async () => {
  const f = await fixture();
  expect(await f.api.overview(f.owner, f.signal)).toEqual(overview());
  const [path, options] = f.fetcher.mock.calls[0];
  expect(path).toBe("/api/v1/me/overview");
  expect(options).toMatchObject({
    method: "POST",
    credentials: "same-origin",
    cache: "no-store",
    redirect: "error",
    body: JSON.stringify({ v: 1 }),
  });
  expect(options?.headers).toMatchObject({
    "content-type": "application/json",
    "x-liar-csrf": "fixture-memory-csrf",
  });
  const h = await fixture(history());
  expect(
    await h.api.history(
      h.owner,
      { kind: "online", opponent: "all", cursor: null },
      h.signal,
    ),
  ).toEqual(history());
  expect(h.fetcher.mock.calls[0][1]?.body).toBe(
    JSON.stringify({ v: 1, kind: "online", opponent: "all", cursor: null }),
  );
});
test("foreign owners, filter mismatches, secret extra fields and invalid summaries cannot become views", async () => {
  for (const value of [
    {
      ...overview(),
      account: {
        ...overview().account,
        id: "a0000000-0000-4000-8000-000000000002",
      },
    },
    { ...overview(), secret_seed: "forbidden" },
    { ...overview(), online: { ...overview().online, win_rate: 2 } },
    {
      ...overview(),
      recent_online: [
        ...overview().recent_online,
        ...Array(3).fill(overview().recent_online[0]),
      ],
    },
  ]) {
    const f = await fixture(value);
    await expect(f.api.overview(f.owner, f.signal)).rejects.toBeInstanceOf(
      MeFailure,
    );
  }
  const f = await fixture({ ...history(), opponent: "bot" });
  await expect(
    f.api.history(
      f.owner,
      { kind: "online", opponent: "all", cursor: null },
      f.signal,
    ),
  ).rejects.toBeInstanceOf(MeFailure);
});
test("unauthorized responses clear authority while unavailable and rate-limited responses are explicit", async () => {
  const f = await fixture();
  f.fetcher.mockResolvedValueOnce(
    new Response(JSON.stringify({ error: "unauthorized" }), {
      status: 401,
      headers: { "content-type": "application/json" },
    }),
  );
  await expect(f.api.overview(f.owner, f.signal)).rejects.toBeTruthy();
  expect(f.auth.read().account).toBeNull();
  const g = await fixture();
  g.fetcher.mockResolvedValueOnce(
    new Response(JSON.stringify({ error: "rate_limited" }), {
      status: 429,
      headers: { "content-type": "application/json" },
    }),
  );
  await expect(g.api.overview(g.owner, g.signal)).rejects.toMatchObject({
    code: "rate_limited",
  });
});

test("daily and current or expired consent views preserve nullable trust without exposing replay bodies", async () => {
  const value = history();
  value.kind = "daily";
  value.online = null;
  value.items = [{ kind: "daily", entry: overview().recent_daily[0] }];
  const f = await fixture(value);
  expect(
    await f.api.history(
      f.owner,
      { kind: "daily", opponent: "all", cursor: null },
      f.signal,
    ),
  ).toEqual(value);
  const timed = structuredClone(value);
  if (timed.items[0].kind !== "daily") throw Error();
  timed.items[0].entry.verified_elapsed_ms = 4000;
  timed.items[0].entry.trust = "verified_time";
  const g = await fixture(timed);
  expect(
    (
      await g.api.history(
        g.owner,
        { kind: "daily", opponent: "all", cursor: null },
        g.signal,
      )
    ).items,
  ).toEqual(timed.items);
  for (const expired of [false, true]) {
    const snapshot = overview();
    snapshot.consent = {
      cmp_available: false,
      status: expired ? "expired" : "current",
      analytics: expired ? "unknown" : "granted",
      advertising: expired ? "unknown" : "denied",
      checked_at: snapshot.observed_at - 1,
      expires_at: snapshot.observed_at + (expired ? 0 : 1),
    };
    const h = await fixture(snapshot);
    expect((await h.api.overview(h.owner, h.signal)).consent).toEqual(
      snapshot.consent,
    );
  }
});
