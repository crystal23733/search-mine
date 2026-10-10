// @vitest-environment node
import { expect, test, vi } from "vitest";
import { createLatestResultHttp, createResultHttp } from "./result-http";
import { createAuth } from "../auth/session";
import { PROVIDERS, type AuthTransport } from "../auth/types";
import { matchId, personalResult } from "../../../test/online-fixture";
const account = { id: matchId, nickname: "player" };
const bootstrap = (session = matchId) => ({
  account,
  session_revision: session,
  csrf: "fresh-proof",
  providers: PROVIDERS.map((p) => ({ provider: p.id, available: true })),
});
async function setup() {
  const transport: AuthTransport = {
    bootstrap: vi.fn(async () => bootstrap()),
    identities: vi.fn(),
    start: vi.fn(),
    nickname: vi.fn(),
    export: vi.fn(),
    erase: vi.fn(),
    unlink: vi.fn(),
    logout: vi.fn(),
  };
  const auth = createAuth(transport, () => true);
  await auth.refresh();
  vi.mocked(transport.bootstrap).mockClear();
  return {
    auth,
    transport,
    owner: { accountId: matchId, revision: auth.revision() },
  };
}
const response = (raw: unknown, status = 200) =>
  new Response(JSON.stringify(raw), {
    status,
    headers: { "content-type": "application/json" },
  });
test("latest discovers one validated result or explicit null using a fixed ID-free POST and fresh proofs", async () => {
  const s = await setup();
  for (const result of [personalResult(), null]) {
    const fetcher = vi.fn(async () => response({ v: 1, result }));
    expect(
      await createLatestResultHttp(s.auth, fetcher)(
        s.owner,
        new AbortController().signal,
      ),
    ).toEqual(result);
    expect(fetcher.mock.calls[0]).toEqual([
      "/api/v1/results/latest",
      expect.objectContaining({
        body: '{"v":1}',
        cache: "no-store",
        credentials: "same-origin",
        redirect: "error",
        headers: {
          "content-type": "application/json",
          "x-liar-csrf": "fresh-proof",
        },
      }),
    ]);
  }
  expect(s.transport.bootstrap).toHaveBeenCalledTimes(4);
  for (const r of [
    response({ error: "unavailable" }, 503),
    response({ error: "not_found" }, 404),
    response({ v: 1, result: { ...personalResult(), seed: "hidden" } }),
    response({ v: 1, result: null, account: "x" }),
    response({ v: 1, result: { ...personalResult(), match_id: "invalid" } }),
  ]) {
    await expect(
      createLatestResultHttp(
        s.auth,
        vi.fn(async () => r),
      )(s.owner, new AbortController().signal),
    ).rejects.toMatchObject({ code: "unavailable" });
  }
  s.auth.dispose();
});
test("latest body decoding stays within authenticated work and session replacement rejects even explicit null", async () => {
  const s = await setup();
  const fetcher = vi.fn(async () => {
    vi.mocked(s.transport.bootstrap).mockResolvedValue(
      bootstrap("22222222-2222-4222-8222-222222222222"),
    );
    return response({ v: 1, result: null });
  });
  await expect(
    createLatestResultHttp(s.auth, fetcher)(
      s.owner,
      new AbortController().signal,
    ),
  ).rejects.toMatchObject({ code: "auth_invalid" });
  expect(s.auth.connected()).toBe(false);
  s.auth.dispose();
});
test("uses production auth before and after the minimal fixed POST and preserves stable authority", async () => {
  const s = await setup(),
    fetcher = vi.fn(async () => response({ v: 1, result: personalResult() }));
  const signal = new AbortController().signal;
  expect(
    await createResultHttp(s.auth, fetcher)(s.owner, matchId, signal),
  ).toEqual(personalResult());
  expect(s.transport.bootstrap).toHaveBeenCalledTimes(2);
  expect(fetcher.mock.calls[0]).toEqual([
    "/api/v1/results",
    expect.objectContaining({
      method: "POST",
      credentials: "same-origin",
      cache: "no-store",
      redirect: "error",
      headers: {
        "content-type": "application/json",
        "x-liar-csrf": "fresh-proof",
      },
      body: JSON.stringify({ v: 1, match_id: matchId }),
      signal: expect.any(AbortSignal),
    }),
  ]);
  expect(s.auth.revision()).toBe(s.owner.revision);
  s.auth.dispose();
});
test("session replacement during a reply never exposes it and a pre-cancelled call does not fetch", async () => {
  const s = await setup();
  const fetcher = vi.fn(async () => {
    vi.mocked(s.transport.bootstrap).mockResolvedValue(
      bootstrap("22222222-2222-4222-8222-222222222222"),
    );
    return response({ v: 1, result: personalResult() });
  });
  const cancelled = new AbortController();
  cancelled.abort();
  await expect(
    createResultHttp(s.auth, fetcher)(s.owner, matchId, cancelled.signal),
  ).rejects.toMatchObject({ code: "cancelled" });
  expect(fetcher).not.toHaveBeenCalled();
  await expect(
    createResultHttp(s.auth, fetcher)(
      s.owner,
      matchId,
      new AbortController().signal,
    ),
  ).rejects.toMatchObject({ code: "auth_invalid" });
  expect(s.auth.connected()).toBe(false);
  s.auth.dispose();
});
test("uniform not_found and unavailable remain distinct, malformed/status-mismatched/large/MIME replies fail closed", async () => {
  const s = await setup();
  const run = (r: Response) =>
    createResultHttp(
      s.auth,
      vi.fn(async () => r),
    )(s.owner, matchId, new AbortController().signal);
  await expect(
    run(response({ error: "not_found" }, 404)),
  ).rejects.toMatchObject({ code: "not_found" });
  await expect(
    run(response({ error: "unavailable" }, 503)),
  ).rejects.toMatchObject({ code: "unavailable" });
  for (const r of [
    response({ error: "not_found" }, 503),
    response({ error: "unknown" }, 404),
    response({ v: 1, result: { ...personalResult(), seed: "hidden" } }),
    new Response(" ".repeat(4097), {
      headers: { "content-type": "application/json" },
    }),
    new Response("{}", { headers: { "content-type": "text/html" } }),
    new Response(new Uint8Array([0xff]), {
      headers: { "content-type": "application/json" },
    }),
  ])
    await expect(run(r)).rejects.toMatchObject({ code: "unavailable" });
  expect(s.auth.connected()).toBe(true);
  await expect(
    run(response({ error: "unauthorized" }, 401)),
  ).rejects.toMatchObject({ code: "auth_required" });
  expect(s.auth.connected()).toBe(false);
  s.auth.dispose();
});
test("caller cancellation releases a partially read stream and a redirected reply fails closed", async () => {
  const s = await setup(),
    caller = new AbortController();
  let reading = false,
    cancelled = false;
  const body = new ReadableStream<Uint8Array>({
    start(controller) {
      controller.enqueue(new TextEncoder().encode('{"v":1,'));
    },
    pull() {
      reading = true;
    },
    cancel() {
      cancelled = true;
    },
  });
  const pending = createResultHttp(
    s.auth,
    vi.fn(
      async () =>
        new Response(body, { headers: { "content-type": "application/json" } }),
    ),
  )(s.owner, matchId, caller.signal);
  const rejected = expect(pending).rejects.toMatchObject({ code: "cancelled" });
  await vi.waitFor(() => expect(reading).toBe(true));
  caller.abort();
  await rejected;
  expect(cancelled).toBe(true);
  const redirected = response({ v: 1, result: personalResult() });
  Object.defineProperty(redirected, "redirected", { value: true });
  await expect(
    createResultHttp(
      s.auth,
      vi.fn(async () => redirected),
    )(s.owner, matchId, new AbortController().signal),
  ).rejects.toMatchObject({ code: "unavailable" });
  expect(s.auth.connected()).toBe(true);
  s.auth.dispose();
});
