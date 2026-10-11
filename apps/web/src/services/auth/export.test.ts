// @vitest-environment node
import { expect, test, vi } from "vitest";
import { createAuth } from "./session";
import { PROVIDERS, AuthError } from "./types";
import {
  decodeGamePage,
  decodeReplayExport,
  decodePersonalDataExport,
  collectGameExport,
  EXPORT_MAX_BYTES,
} from "./game-export";
import { personalResult } from "../../../test/online-fixture";
import type { ExportRecord } from "@liar/protocol";
const account = { id: "a0000000-0000-4000-8000-000000000001", nickname: null };
const revision = "b0000000-0000-4000-8000-000000000001";
function fixture() {
  const bootstrap = {
    account,
    session_revision: revision,
    csrf: "memory-proof",
    providers: PROVIDERS.map((p) => ({ provider: p.id, available: true })),
  };
  const t = {
    bootstrap: vi.fn().mockResolvedValue(bootstrap),
    identities: vi.fn().mockResolvedValue([]),
    start: vi.fn(),
    nickname: vi.fn(),
    erase: vi.fn(),
    unlink: vi.fn(),
    logout: vi.fn(),
    export: vi.fn().mockResolvedValue({
      account,
      created_at: 1,
      last_seen_at: 2,
      identities: [],
    }),
    gameExport: vi.fn().mockImplementation(async (input) => ({
      v: 1,
      account_id: account.id,
      stream: input.stream,
      records: [],
      next: null,
    })),
    replayExport: vi.fn(),
  };
  return { t, bootstrap, auth: createAuth(t) };
}
test("explicit export includes both game streams without requiring a nickname and proves the session again", async () => {
  const { t, auth } = fixture();
  await auth.refresh();
  const before = t.bootstrap.mock.calls.length;
  const result = await auth.export(account.id);
  expect(result).toEqual({
    ok: true,
    value: {
      v: 1,
      account: { account, created_at: 1, last_seen_at: 2, identities: [] },
      games: [],
      replays: [],
    },
  });
  expect(t.gameExport.mock.calls.map((c) => c[0])).toEqual(
    ["online", "daily"].map((stream) => ({
      v: 1,
      account_id: account.id,
      session_revision: revision,
      stream,
      after: null,
    })),
  );
  expect(t.bootstrap.mock.calls.length - before).toBe(2);
});

const id = (n: number) =>
  `00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;
function online(n: number): ExportRecord {
  return {
    kind: "online",
    result: { ...personalResult(), match_id: id(n) },
    recorded_at: 100,
    retention_started_at: 99,
  };
}
const metadata = {
  date: "2026-10-07",
  seed_version: 1,
  mode: "solo-v1",
  seed: "42",
  rules_hash: "a".repeat(64),
  solver_version: 1,
  rng_version: 1,
};
const daily = (n: number): Extract<ExportRecord, { kind: "daily" }> => ({
  kind: "daily",
  completion_id: id(n),
  attempt_id: id(n + 100),
  metadata,
  opened_safe: 216,
  mistakes: 0,
  replayed_elapsed_ms: 3200,
  completed_at: 100,
  replay_available: true,
});
const replay = () => ({
  v: 1,
  metadata,
  inputs: [
    {
      time_ms: 3000,
      input: {
        v: 1,
        command_id: 1,
        client_seq: 1,
        action: { type: "open", cell: 8 },
      },
    },
  ],
  final_time_ms: 3200,
});
const page = (
  stream = "online",
  records: ExportRecord[] = [],
  next: string | null = null,
) => ({ v: 1, account_id: account.id, stream, records, next });
test("all pages and live replay references are collected in order with one immutable proof", async () => {
  const { t, auth } = fixture();
  await auth.refresh();
  t.gameExport
    .mockResolvedValueOnce(
      page(
        "online",
        Array.from({ length: 50 }, (_, n) => online(n + 1)),
        id(50),
      ),
    )
    .mockResolvedValueOnce(page("online", [online(51)]))
    .mockResolvedValueOnce(
      page("daily", [daily(1), { ...daily(2), replay_available: false }]),
    );
  t.replayExport.mockResolvedValue({
    v: 1,
    account_id: account.id,
    completion_id: id(1),
    replay: replay(),
  });
  const result = await auth.export(account.id);
  expect(result.ok).toBe(true);
  if (!result.ok) throw Error();
  expect(result.value.games).toHaveLength(53);
  expect(result.value.replays).toHaveLength(1);
  expect(t.gameExport.mock.calls[1][0].after).toBe(id(50));
  expect(t.replayExport).toHaveBeenCalledWith(
    {
      v: 1,
      account_id: account.id,
      session_revision: revision,
      completion_id: id(1),
    },
    "memory-proof",
    expect.any(AbortSignal),
  );
});
test("normal missing raw stays null while errors never become a complete file", async () => {
  for (const fails of [false, true]) {
    const { t, auth } = fixture();
    await auth.refresh();
    t.gameExport
      .mockResolvedValueOnce(page())
      .mockResolvedValueOnce(page("daily", [daily(1)]));
    if (fails)
      t.replayExport.mockRejectedValue(new AuthError("auth_unavailable"));
    else
      t.replayExport.mockResolvedValue({
        v: 1,
        account_id: account.id,
        completion_id: id(1),
        replay: null,
      });
    const result = await auth.export(account.id);
    expect(result.ok).toBe(!fails);
    if (result.ok) expect(result.value.replays[0].replay).toBeNull();
  }
});
test("wrong owner stream cursor record or replay association prevents completion", async () => {
  const invalidPages = [
    { ...page(), account_id: id(99) },
    { ...page(), stream: "daily" },
    page("online", [online(2), online(1)]),
    page("online", [online(1), online(1)]),
    page("online", [online(1)], id(1)),
    page("online", [daily(1)]),
    { ...page(), seed: "secret" },
  ];
  for (const invalid of invalidPages) {
    const { t, auth } = fixture();
    await auth.refresh();
    t.gameExport.mockResolvedValueOnce(invalid);
    expect((await auth.export(account.id)).ok).toBe(false);
  }
  for (const invalid of [
    { v: 1, account_id: id(9), completion_id: id(1), replay: null },
    { v: 1, account_id: account.id, completion_id: id(9), replay: null },
    {
      v: 1,
      account_id: account.id,
      completion_id: id(1),
      replay: { ...replay(), metadata: { ...metadata, seed: "43" } },
    },
  ]) {
    const { t, auth } = fixture();
    await auth.refresh();
    t.gameExport
      .mockResolvedValueOnce(page())
      .mockResolvedValueOnce(page("daily", [daily(1)]));
    t.replayExport.mockResolvedValue(invalid);
    expect((await auth.export(account.id)).ok).toBe(false);
  }
  const { t, auth } = fixture();
  await auth.refresh();
  t.gameExport
    .mockResolvedValueOnce(
      page(
        "online",
        Array.from({ length: 50 }, (_, n) => online(n + 1)),
        id(50),
      ),
    )
    .mockResolvedValueOnce(page("online", [online(1)]));
  expect((await auth.export(account.id)).ok).toBe(false);
});
test("final proof rejects same-account session rotation and delayed invalidated collection", async () => {
  const { t, bootstrap, auth } = fixture();
  await auth.refresh();
  t.bootstrap
    .mockResolvedValueOnce(bootstrap)
    .mockResolvedValueOnce({ ...bootstrap, session_revision: id(99) });
  expect(await auth.export(account.id)).toEqual({
    ok: false,
    code: "auth_invalid",
  });
  const f = fixture();
  await f.auth.refresh();
  let reply!: (value: unknown) => void;
  f.t.gameExport.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        reply = resolve;
      }),
  );
  const old = f.auth.export(account.id);
  await vi.waitFor(() => expect(f.t.gameExport).toHaveBeenCalledTimes(1));
  f.auth.invalidate();
  reply(page());
  expect((await old).ok).toBe(false);
  expect(f.auth.read().account).toBeNull();
});
test("strict wire decoders reject unknown fields numeric ranges versions and excessive pages", () => {
  for (const invalid of [
    { ...page(), v: 2 },
    page(
      "online",
      Array.from({ length: 51 }, (_, n) => online(n + 1)),
    ),
    { ...page(), records: "bad" },
    { ...page(), next: id(1) },
    page("online", [{ ...online(1), recorded_at: -1 } as ExportRecord]),
    page("daily", [{ ...daily(1), opened_safe: 0 } as ExportRecord]),
    page("daily", [
      {
        ...daily(1),
        metadata: { ...metadata, date: "2026-02-30" },
      } as ExportRecord,
    ]),
    { ...page(), padding: "x".repeat(65536) },
  ])
    expect(() => decodeGamePage(invalid)).toThrow();
  for (const raw of [
    { ...replay(), v: 2 },
    { ...replay(), extra: true },
    {
      ...replay(),
      inputs: [
        {
          time_ms: 0,
          input: {
            v: 1,
            command_id: 0,
            client_seq: 1,
            action: { type: "attack" },
          },
        },
      ],
    },
    {
      ...replay(),
      inputs: [
        {
          time_ms: 0,
          input: {
            v: 1,
            command_id: 1,
            client_seq: 1,
            action: { type: "open", cell: -1 },
          },
        },
      ],
    },
    { ...replay(), metadata: { ...metadata, seed: "18446744073709551616" } },
    {
      ...replay(),
      inputs: Array.from({ length: 8193 }, () => replay().inputs[0]),
    },
  ])
    expect(() =>
      decodeReplayExport({
        v: 1,
        account_id: account.id,
        completion_id: id(1),
        replay: raw,
      }),
    ).toThrow();
  for (const action of [
    { type: "attack" },
    { type: "flag", cell: 1 },
    { type: "accuse", cell: 1 },
  ]) {
    const raw = replay();
    raw.inputs[0].input.action =
      action as (typeof raw.inputs)[0]["input"]["action"];
    expect(
      decodeReplayExport({
        v: 1,
        account_id: account.id,
        completion_id: id(1),
        replay: raw,
      }).replay,
    ).not.toBeNull();
  }
  const bundle = {
    v: 1,
    account: { account, created_at: 1, last_seen_at: 2, identities: [] },
    games: [daily(1)],
    replays: [
      { v: 1, account_id: account.id, completion_id: id(1), replay: null },
    ],
  };
  expect(decodePersonalDataExport(bundle).games).toHaveLength(1);
  for (const bad of [
    { ...bundle, replays: [] },
    { ...bundle, games: [daily(1), daily(1)] },
    { ...bundle, replays: [...bundle.replays, ...bundle.replays] },
    { ...bundle, replays: [{ ...bundle.replays[0], account_id: id(9) }] },
    { ...bundle, replays: [{ ...bundle.replays[0], completion_id: id(9) }] },
    { ...bundle, extra: "x".repeat(EXPORT_MAX_BYTES) },
  ])
    expect(() => decodePersonalDataExport(bad)).toThrow();
});
test("aborted collection does not send any request", async () => {
  const { t } = fixture(),
    controller = new AbortController();
  controller.abort();
  await expect(
    collectGameExport(
      t,
      { account, created_at: 1, last_seen_at: 2, identities: [] },
      revision,
      "proof",
      controller.signal,
    ),
  ).rejects.toThrow();
  expect(t.gameExport).not.toHaveBeenCalled();
});

test("download sink executes only for the current completion and sink errors stay explicit", async () => {
  const { auth } = fixture();
  await auth.refresh();
  const sink = vi.fn(async () => {});
  expect((await auth.exportTo(account.id, sink)).ok).toBe(true);
  expect(sink).toHaveBeenCalledTimes(1);
  expect(
    await auth.exportTo(account.id, async () => {
      throw Error("download unavailable");
    }),
  ).toEqual({ ok: false, code: "auth_unavailable" });
  const f = fixture();
  await f.auth.refresh();
  f.t.bootstrap
    .mockResolvedValueOnce(f.bootstrap)
    .mockResolvedValueOnce({ ...f.bootstrap, session_revision: id(99) });
  const stale = vi.fn(async () => {});
  expect((await f.auth.exportTo(account.id, stale)).ok).toBe(false);
  expect(stale).not.toHaveBeenCalled();
});
test("page and raw requests are paced and cancellation interrupts waiting before another request", async () => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
  try {
    const f = fixture();
    await f.auth.refresh();
    const sent: number[] = [];
    f.t.gameExport.mockImplementation(async (input) => {
      sent.push(performance.now());
      return input.stream === "online" ? page() : page("daily", [daily(1)]);
    });
    f.t.replayExport.mockImplementation(async () => {
      sent.push(performance.now());
      return {
        v: 1,
        account_id: account.id,
        completion_id: id(1),
        replay: null,
      };
    });
    const task = f.auth.export(account.id);
    await vi.runAllTimersAsync();
    expect((await task).ok).toBe(true);
    expect(sent).toHaveLength(3);
    expect(sent[1] - sent[0]).toBeGreaterThanOrEqual(100);
    expect(sent[2] - sent[1]).toBeGreaterThanOrEqual(100);
    const t = fixture();
    await t.auth.refresh();
    const cancelled = t.auth.export(account.id);
    for (let i = 0; i < 20; i++) await Promise.resolve();
    expect(t.t.gameExport).toHaveBeenCalledTimes(1);
    t.auth.invalidate();
    expect((await cancelled).ok).toBe(false);
    expect(t.t.gameExport).toHaveBeenCalledTimes(1);
  } finally {
    vi.useRealTimers();
  }
});
