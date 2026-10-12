// @vitest-environment node
import { afterEach, expect, test, vi } from "vitest";
import { DEFAULT_RULES, type DailyReplay } from "@liar/protocol";
import native from "../../../../../tests/fixtures/native-wasm.json";
import { createDailyHttp } from "./http";
import { createAuthenticatedRequests } from "../auth/requests";
import {
  createDailyRecords,
  dailyId,
  type DailyRecord,
} from "../daily-records";
import { createPendingSubmissions } from "../pending-submissions";
const account = "a0000000-0000-4000-8000-000000000001",
  other = "a0000000-0000-4000-8000-000000000002";
const attempt = "b0000000-0000-4000-8000-000000000001",
  ticket = "c0000000-0000-4000-8000-000000000001";
const replay = native.daily.replay as DailyReplay,
  metadata = replay.metadata;
const definition = { v: 1, metadata, rules: DEFAULT_RULES };
const receipt = {
  completion_id: ticket,
  attempt_id: attempt,
  metadata,
  opened_safe: 200,
  mistakes: 0,
  replayed_elapsed_ms: 1000,
  verified_elapsed_ms: null as number | null,
  completed_at: 1800000000,
};
const saved = {
  v: 1,
  account_id: account,
  attempt_id: attempt,
  disposition: "official",
  trust: "verified_completion",
  receipt,
};
const empty = {
  v: 1,
  metadata: null,
  time: { entries: [], has_more: false },
  completion: { entries: [], has_more: false },
};
const owner = { accountId: account, revision: 1 };
const record: DailyRecord = {
  v: 1,
  id: dailyId(metadata),
  status: "unverified",
  attempt_id: attempt,
  metadata,
  elapsed_ms: replay.final_time_ms,
  mistakes: 0,
  replay,
};
const json = (raw: unknown, status = 200) =>
  new Response(JSON.stringify(raw), {
    status,
    headers: { "content-type": "application/json" },
  });
function setup(fetcher: typeof fetch) {
  let cookie = account;
  const bootstrap = vi.fn(async () => ({
    account: { id: cookie, nickname: "player" },
    csrf: "fresh-proof",
    session_revision: ticket,
    providers: [],
  }));
  const invalidated = vi.fn();
  const auth = createAuthenticatedRequests({
    authority: () => ({ ...owner, sessionId: ticket, generation: 0 }),
    bootstrap,
    invalidated,
  });
  return {
    api: createDailyHttp(auth, fetcher),
    bootstrap,
    invalidated,
    auth,
    switchCookie: () => {
      cookie = other;
    },
  };
}
afterEach(() => vi.useRealTimers());
test("wire replay bounds reject extra fields, invalid commands and more than8192 without sending", async () => {
  const fetcher = vi.fn(async () => json(saved));
  const s = setup(fetcher);
  const input = {
    v: 1,
    command_id: 1,
    client_seq: 1,
    action: { type: "open" as const, cell: 1 },
  };
  for (const invalid of [
    { ...replay, extra: 1 },
    {
      ...replay,
      inputs: Array.from({ length: 8193 }, () => ({ time_ms: 3001, input })),
    },
    {
      ...replay,
      inputs: [{ time_ms: 3001, input: { ...input, command_id: 0 } }],
    },
    {
      ...replay,
      inputs: [
        {
          time_ms: 3001,
          input: { ...input, action: { type: "open", cell: 65536 } },
        },
      ],
    },
    { ...replay, final_time_ms: 4294967296 },
  ]) {
    await expect(
      s.api.submitTimed(owner, attempt, null, invalid as DailyReplay),
    ).rejects.toThrow();
  }
  expect(fetcher).not.toHaveBeenCalled();
  for (const action of [
    { type: "attack" as const },
    { type: "flag" as const, cell: 1 },
    { type: "accuse" as const, cell: 1 },
  ])
    expect(
      await s.api.submitTimed(owner, attempt, null, {
        ...replay,
        inputs: [{ time_ms: 3001, input: { ...input, action } }],
      }),
    ).toEqual(saved);
});
test("all authenticated operations bind opaque memory revision, identity and fresh pre/post proofs", async () => {
  const start = {
    ...definition,
    account_id: account,
    attempt_id: attempt,
    ticket_id: ticket,
    ttl_ms: 243000,
    heartbeat_interval_ms: 15000,
  };
  const heartbeat = {
    v: 1,
    account_id: account,
    ticket_id: ticket,
    ack: { sequence: 1, elapsed_ms: 15000 },
  };
  const replies = [
    start,
    heartbeat,
    definition,
    saved,
    { v: 1, account_id: account, trust: "verified_completion", receipt },
    { ...heartbeat, ack: null },
  ];
  const fetcher = vi.fn<typeof fetch>(async () => json(replies.shift()));
  const s = setup(fetcher);
  expect(await s.api.start(owner, attempt)).toEqual(start);
  expect(await s.api.heartbeat(owner, ticket, 1)).toEqual(heartbeat);
  expect(await s.api.definition(owner, metadata.date, 1)).toEqual(definition);
  expect(await s.api.submitTimed(owner, attempt, ticket, replay)).toEqual(
    saved,
  );
  expect((await s.api.own(owner, metadata)).receipt).toEqual(receipt);
  expect((await s.api.heartbeat(owner, ticket, 1)).ack).toBeNull();
  expect(s.bootstrap).toHaveBeenCalledTimes(12);
  expect(fetcher.mock.calls.map((c) => c[0])).toEqual([
    "/api/v1/daily/start",
    "/api/v1/daily/heartbeat",
    "/api/v1/daily/definition",
    "/api/v1/daily/submit-timed",
    "/api/v1/daily/own",
    "/api/v1/daily/heartbeat",
  ]);
  for (const call of fetcher.mock.calls as unknown as [string, RequestInit][]) {
    expect(call[1]).toMatchObject({
      credentials: "same-origin",
      cache: "no-store",
      redirect: "error",
      method: "POST",
      headers: { "x-liar-csrf": "fresh-proof" },
    });
    expect(JSON.parse(call[1].body as string)).toMatchObject({
      account_id: account,
      session_revision: ticket,
    });
  }
});
test("Practice keeps original receipt attempt, nullable time remains explicit, own null is a normal result", async () => {
  const practice = {
    ...saved,
    disposition: "practice",
    receipt: { ...receipt, attempt_id: other, verified_elapsed_ms: 2000 },
    trust: "verified_time",
  };
  const fetcher = vi.fn(async () => json(practice));
  const s = setup(fetcher);
  expect(await s.api.submitTimed(owner, attempt, null, replay)).toEqual(
    practice,
  );
  fetcher.mockImplementation(async () =>
    json({
      v: 1,
      account_id: account,
      trust: "verified_completion",
      receipt: null,
    }),
  );
  expect((await s.api.own(owner, metadata)).receipt).toBeNull();
});
test("malformed identity, metadata, rules, trust and receipt never become accepted", async () => {
  const start = {
    ...definition,
    account_id: account,
    attempt_id: attempt,
    ticket_id: ticket,
    ttl_ms: 243000,
    heartbeat_interval_ms: 15000,
  };
  const cases: [
    unknown,
    (api: ReturnType<typeof createDailyHttp>) => Promise<unknown>,
  ][] = [];
  const submit = (api: ReturnType<typeof createDailyHttp>) =>
    api.submitTimed(owner, attempt, null, replay);
  for (const raw of [
    { ...saved, account_id: other },
    { ...saved, attempt_id: other },
    { ...saved, secret: 1 },
    { ...saved, receipt: { ...receipt, attempt_id: other } },
    { ...saved, receipt: { ...receipt, verified_elapsed_ms: undefined } },
    { ...saved, trust: "verified_time" },
    {
      ...saved,
      trust: "verified_time",
      receipt: { ...receipt, verified_elapsed_ms: 999 },
    },
    { ...saved, receipt: { ...receipt, metadata: { ...metadata, seed: "2" } } },
    {
      ...saved,
      receipt: {
        ...receipt,
        completion_id: "00000000-0000-0000-0000-000000000000",
      },
    },
  ])
    cases.push([raw, submit]);
  for (const raw of [
    { ...start, ticket_id: "bad" },
    { ...start, attempt_id: other },
    { ...start, account_id: other },
    { ...start, rules: { ...DEFAULT_RULES, hash: "a".repeat(64) } },
  ])
    cases.push([raw, (a) => a.start(owner, attempt)]);
  for (const raw of [
    { ...definition, metadata: { ...metadata, date: "2026-02-30" } },
    { ...definition, metadata: { ...metadata, seed: "18446744073709551616" } },
    { ...definition, metadata: { ...metadata, rng_version: 65536 } },
    { ...definition, rules: { ...DEFAULT_RULES, extra: 1 } },
    { ...definition, metadata: { ...metadata, date: "2026-01-01" } },
    { ...definition, metadata: { ...metadata, seed_version: 2 } },
  ])
    cases.push([raw, (a) => a.definition(owner, metadata.date, 1)]);
  for (const raw of [
    { v: 1, account_id: other, ticket_id: ticket, ack: null },
    { v: 1, account_id: account, ticket_id: other, ack: null },
    {
      v: 1,
      account_id: account,
      ticket_id: ticket,
      ack: { sequence: 2, elapsed_ms: 1 },
    },
  ])
    cases.push([raw, (a) => a.heartbeat(owner, ticket, 1)]);
  cases.push([
    { v: 1, account_id: other, trust: "verified_completion", receipt: null },
    (a) => a.own(owner, metadata),
  ]);
  cases.push([
    { v: 1, account_id: account, trust: "verified_time", receipt: null },
    (a) => a.own(owner, metadata),
  ]);
  for (const [raw, run] of cases)
    await expect(
      run(setup(vi.fn(async () => json(raw))).api),
    ).rejects.toMatchObject({ code: "unavailable" });
});
test("only exact error status/code pairs classify terminal expiry; authentication invalidates", async () => {
  for (const [code, status, expected] of [
    ["daily_invalid", 400, "invalid"],
    ["daily_conflict", 409, "conflict"],
    ["daily_expired", 410, "expired"],
    ["daily_rate_limited", 429, "rate_limited"],
    ["daily_unavailable", 503, "unavailable"],
    ["auth_required", 401, "auth_required"],
    ["daily_expired", 400, "unavailable"],
    ["daily_unsupported", 400, "unavailable"],
  ] as const) {
    const s = setup(vi.fn(async () => json({ code }, status)));
    await expect(s.api.own(owner, metadata)).rejects.toMatchObject({
      code: expected,
    });
    expect(s.invalidated).toHaveBeenCalledTimes(
      code === "auth_required" ? 1 : 0,
    );
  }
});
test("pending accepted, expired and accepted-definition mismatch all require post-proof; local records survive", async () => {
  for (const outcome of ["accepted", "expired", "unsupported"] as const)
    for (const changed of [false, true]) {
      const fetcher = vi.fn(async (path: RequestInfo | URL) => {
        if (String(path).endsWith("definition"))
          return json(
            outcome === "unsupported"
              ? { ...definition, metadata: { ...metadata, seed: "2" } }
              : definition,
          );
        if (changed) s.switchCookie();
        return outcome === "expired"
          ? json({ code: "daily_expired" }, 410)
          : json(saved);
      });
      const s = setup(fetcher);
      if (outcome === "unsupported" && changed)
        fetcher.mockImplementation(async () => {
          s.switchCookie();
          return json({ ...definition, metadata: { ...metadata, seed: "2" } });
        });
      const records = createDailyRecords();
      await records.saveFirst(record);
      const queue = createPendingSubmissions(
        records,
        {
          account: () => account,
          revision: () => 1,
          connected: () => true,
          subscribe: () => () => {},
        },
        {
          submit: (r, id, signal) =>
            s.api.submitPending({ accountId: id, revision: 1 }, r, signal),
        },
      );
      const result = await queue.flush(account);
      expect(result.removed).toBe(changed ? 0 : 1);
      expect((await records.pending()).value).toHaveLength(changed ? 1 : 0);
      expect((await records.list()).value).toHaveLength(1);
      expect(s.bootstrap).toHaveBeenCalledTimes(2);
    }
});
test("expiry during definition still checks post-proof and metadata disagreement skips submission", async () => {
  const fetcher = vi.fn(async () => json({ code: "daily_expired" }, 410));
  const s = setup(fetcher);
  expect(
    await s.api.submitPending(owner, record, new AbortController().signal),
  ).toBe("expired");
  expect(s.bootstrap).toHaveBeenCalledTimes(2);
  fetcher.mockImplementation(async () => json(definition));
  expect(
    await s.api.submitPending(
      owner,
      {
        ...record,
        replay: { ...replay, metadata: { ...metadata, seed: "2" } },
      },
      new AbortController().signal,
    ),
  ).toBe("unsupported");
  expect(fetcher).toHaveBeenCalledTimes(2);
});
test("generic invalid, conflict, malformed and unavailable retain pending candidates", async () => {
  for (const reply of [
    json({ code: "daily_invalid" }, 400),
    json({ code: "daily_conflict" }, 409),
    json({ code: "daily_expired", unexpected: 1 }, 410),
    json({ code: "daily_unavailable" }, 503),
    json({ ...saved, account_id: other }),
  ]) {
    const s = setup(
      vi.fn(async (path: RequestInfo | URL) =>
        String(path).endsWith("definition") ? json(definition) : reply,
      ),
    );
    const records = createDailyRecords();
    await records.saveFirst(record);
    const queue = createPendingSubmissions(
      records,
      {
        account: () => account,
        revision: () => 1,
        connected: () => true,
        subscribe: () => () => {},
      },
      {
        submit: (r, id, signal) =>
          s.api.submitPending({ accountId: id, revision: 1 }, r, signal),
      },
    );
    expect((await queue.flush(account)).retained).toBe(true);
    expect((await records.pending()).value).toHaveLength(1);
  }
});
test("public rankings omit credentials, CSRF and proofs; normal empty and both pages decode", async () => {
  const fetcher = vi.fn(async () => json(empty));
  const s = setup(fetcher);
  expect(await s.api.rankings(metadata.date, 1)).toEqual(empty);
  expect(s.bootstrap).not.toHaveBeenCalled();
  expect(fetcher.mock.calls[0]).toEqual([
    "/api/v1/daily/rankings",
    expect.objectContaining({
      credentials: "omit",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ v: 1, date: metadata.date, seed_version: 1 }),
    }),
  ]);
  const populated = {
    ...empty,
    metadata,
    time: {
      entries: [
        { position: 1, nickname: "player", mistakes: 0, elapsed_ms: 2000 },
      ],
      has_more: false,
    },
    completion: {
      entries: [{ position: 1, nickname: "player", mistakes: 0 }],
      has_more: false,
    },
  };
  fetcher.mockImplementation(async () => json(populated));
  expect(await s.api.rankings(metadata.date, 1)).toEqual(populated);
  for (const bad of [
    { ...populated, metadata: null },
    { ...populated, metadata: { ...metadata, date: "2026-01-01" } },
    { ...populated, time: { ...populated.time, has_more: true } },
    {
      ...populated,
      completion: {
        entries: [{ position: 2, nickname: "p", mistakes: 0 }],
        has_more: false,
      },
    },
    {
      ...populated,
      completion: {
        entries: [
          { position: 1, nickname: "p", mistakes: 0, account_id: account },
        ],
        has_more: false,
      },
    },
    {
      ...populated,
      time: {
        entries: Array.from({ length: 51 }, (_, i) => ({
          position: i + 1,
          nickname: "p",
          mistakes: 0,
          elapsed_ms: 1,
        })),
        has_more: true,
      },
    },
  ]) {
    fetcher.mockImplementation(async () => json(bad));
    await expect(s.api.rankings(metadata.date, 1)).rejects.toMatchObject({
      code: "unavailable",
    });
  }
});
test("public capacity, ten-second deadline and ignored fetch cancellation release request slots", async () => {
  vi.useFakeTimers();
  const s = setup(vi.fn(() => new Promise<Response>(() => {})));
  const one = s.api.rankings(metadata.date, 1),
    two = s.api.rankings(metadata.date, 1);
  const rejected = Promise.allSettled([one, two]);
  await expect(s.api.rankings(metadata.date, 1)).rejects.toMatchObject({
    code: "capacity",
  });
  await vi.advanceTimersByTimeAsync(10000);
  expect((await rejected).every((r) => r.status === "rejected")).toBe(true);
  const abort = new AbortController();
  const next = s.api.rankings(metadata.date, 1, abort.signal);
  const settled = expect(next).rejects.toMatchObject({ code: "cancelled" });
  abort.abort();
  await settled;
  await expect(
    s.api.rankings(metadata.date, 1, abort.signal),
  ).rejects.toMatchObject({ code: "cancelled" });
});
test("bounded JSON rejects wrong media, redirects, invalid UTF-8, oversize chunks and network errors", async () => {
  let cancelled = 0;
  const oversized = new Response(
    new ReadableStream<Uint8Array>({
      start(c) {
        c.enqueue(new Uint8Array(65537));
      },
      cancel() {
        cancelled++;
      },
    }),
    { headers: { "content-type": "application/json" } },
  );
  const redirect = json(empty);
  Object.defineProperty(redirect, "redirected", { value: true });
  for (const reply of [
    new Response("{}"),
    redirect,
    new Response(new Uint8Array([0xff]), {
      headers: { "content-type": "application/json" },
    }),
    oversized,
    new Response(null, { headers: { "content-type": "application/json" } }),
    json(empty, 201),
  ])
    await expect(
      setup(vi.fn(async () => reply)).api.rankings(metadata.date, 1),
    ).rejects.toMatchObject({ code: "unavailable" });
  expect(cancelled).toBe(1);
  await expect(
    setup(
      vi.fn(async () => {
        throw Error("offline");
      }),
    ).api.rankings(metadata.date, 1),
  ).rejects.toMatchObject({ code: "unavailable" });
});
test("cancellation while streaming cancels reader and authenticated ignored fetch expires without stale acceptance", async () => {
  const abort = new AbortController();
  let cancelled = 0;
  const body = new ReadableStream<Uint8Array>({
    cancel() {
      cancelled++;
    },
  });
  const api = setup(
    vi.fn(
      async () =>
        new Response(body, { headers: { "content-type": "application/json" } }),
    ),
  ).api;
  const pending = api.rankings(metadata.date, 1, abort.signal);
  const assertion = expect(pending).rejects.toMatchObject({
    code: "cancelled",
  });
  await vi.waitFor(() => expect(body.locked).toBe(true));
  abort.abort();
  await assertion;
  expect(cancelled).toBe(1);
  vi.useFakeTimers();
  const s = setup(vi.fn(() => new Promise<Response>(() => {})));
  const hanging = s.api.own(owner, metadata);
  const timed = expect(hanging).rejects.toMatchObject({ code: "timeout" });
  await vi.advanceTimersByTimeAsync(10000);
  await timed;
});
