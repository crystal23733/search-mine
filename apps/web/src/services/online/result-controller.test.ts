// @vitest-environment node
import { afterEach, expect, test, vi } from "vitest";
import type { OnlineEvent, PersonalResult } from "@liar/protocol";
import { OnlineController, type OnlineAuth } from "./controller";
import { OnlineFailure, type OnlinePort } from "./types";
import { matchId, personalResult, view } from "../../../test/online-fixture";
afterEach(() => vi.useRealTimers());
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}
async function setup() {
  let account: string | null = matchId,
    revision = 1,
    listener = () => {};
  let candidate: { accountId: string; revision: number } | null = null;
  const auth: OnlineAuth = {
    account: () => account,
    revision: () => revision,
    connected: () => account !== null,
    recoveryOwner: () => candidate,
    resume: vi.fn(),
    execute: vi.fn(),
    subscribe: (fn) => {
      listener = fn;
      return () => {};
    },
    invalidate: () => {
      account = null;
      candidate = null;
      revision++;
      listener();
    },
  };
  let event: (e: OnlineEvent) => void = () => {},
    closed: (e: OnlineFailure) => void = () => {};
  const connection = {
    listen: vi.fn((e, c) => {
      event = e;
      closed = c;
    }),
    send: vi.fn(),
    close: vi.fn(),
  };
  const port: OnlinePort = {
    result: vi.fn(async () => personalResult()),
    lobby: vi.fn(
      async () =>
        ({
          v: 1,
          server_time_ms: 0,
          state: {
            type: "matched",
            match_id: matchId,
            own_seat: 0,
            opponent: "human",
          },
        }) as const,
    ),
    connect: vi.fn(async () => connection),
  };
  const controller = new OnlineController(auth, port);
  await controller.start();
  let seq = 0;
  const emit = (payload: OnlineEvent["payload"]) =>
    event({
      v: 1,
      match_id: matchId,
      server_seq: ++seq,
      server_time_ms: 0,
      payload,
    });
  emit({
    type: "snapshot",
    session_epoch: 1,
    last_client_seq: 0,
    view: view(),
  });
  return {
    auth,
    controller,
    port,
    connection,
    emit,
    fail: () => closed(new OnlineFailure("not_matched")),
    suspend: () => {
      candidate = { accountId: matchId, revision };
      account = null;
      listener();
    },
    replace: () => {
      revision++;
      listener();
    },
  };
}
test("only an explicit failed match lookup publishes the stored personal result and discards the stale board", async () => {
  const s = await setup();
  await s.controller.lookupResult();
  expect(s.port.result).not.toHaveBeenCalled();
  s.fail();
  expect(s.controller.canLookupResult()).toBe(true);
  await s.controller.lookupResult();
  expect(s.port.result).toHaveBeenCalledWith(
    { accountId: matchId, revision: 1 },
    matchId,
    expect.any(AbortSignal),
  );
  expect(s.controller.read()).toMatchObject({
    status: "ended",
    personalResult: personalResult(),
    view: null,
    opponent: null,
    recording: "saved",
    error: null,
    pending: 0,
    resultLookup: "idle",
  });
  expect(s.controller.canLookupResult()).toBe(false);
  s.controller.dispose();
});
test("one active lookup, uniform404 and outages never manufacture a result and can be retried", async () => {
  const s = await setup();
  s.fail();
  const late = deferred<PersonalResult>();
  vi.mocked(s.port.result).mockReturnValueOnce(late.promise);
  const pending = s.controller.lookupResult();
  await s.controller.lookupResult();
  expect(s.port.result).toHaveBeenCalledTimes(1);
  expect(s.controller.read().resultLookup).toBe("loading");
  late.resolve(personalResult());
  await pending;
  await s.controller.start();
  s.fail();
  for (const [code, expected] of [
    ["not_found", "not_found"],
    ["unavailable", "unavailable"],
    ["rate_limited", "unavailable"],
  ]) {
    vi.mocked(s.port.result).mockRejectedValueOnce(new OnlineFailure(code));
    await s.controller.lookupResult();
    expect(s.controller.read()).toMatchObject({
      personalResult: null,
      resultLookup: expected,
      status: "error",
    });
    expect(s.controller.canLookupResult()).toBe(true);
  }
  s.controller.dispose();
});
test.each(["start", "dispose", "replace", "suspend"] as const)(
  "%s aborts an active lookup and ignored late replies cannot publish",
  async (action) => {
    const s = await setup();
    s.fail();
    const late = deferred<PersonalResult>();
    vi.mocked(s.port.result).mockReturnValueOnce(late.promise);
    const pending = s.controller.lookupResult();
    const signal = vi.mocked(s.port.result).mock.calls[0]![2];
    if (action === "start" || action === "dispose")
      await s.controller[action]();
    else s[action]();
    expect(signal.aborted).toBe(true);
    late.resolve(personalResult());
    await pending;
    expect(s.controller.read().personalResult).toBeNull();
    if (action === "suspend") {
      await s.controller.lookupResult();
      expect(s.port.result).toHaveBeenCalledTimes(1);
    }
    s.controller.dispose();
  },
);
test("WS terminal result wins before HTTP commit and a saved WS result never queries", async () => {
  const s = await setup(),
    terminal = view();
  terminal.result = { reason: "timeout", outcome: "loss", completed: true };
  terminal.phase = "finished";
  s.emit({ type: "match_end", view: terminal, recording: "pending" });
  const late = deferred<PersonalResult>();
  vi.mocked(s.port.result).mockReturnValueOnce(late.promise);
  const pending = s.controller.lookupResult();
  const signal = vi.mocked(s.port.result).mock.calls[0]![2];
  s.emit({ type: "match_end", view: terminal, recording: "saved" });
  expect(signal.aborted).toBe(true);
  late.resolve(personalResult());
  await pending;
  expect(s.controller.read()).toMatchObject({
    personalResult: null,
    recording: "saved",
    view: terminal,
  });
  await s.controller.lookupResult();
  expect(s.port.result).toHaveBeenCalledTimes(1);
  s.controller.dispose();
});
test("a newer lookup owns the reused slot when the cancelled older transport completes last", async () => {
  const s = await setup();
  s.fail();
  const old = deferred<PersonalResult>(),
    fresh = deferred<PersonalResult>();
  vi.mocked(s.port.result)
    .mockReturnValueOnce(old.promise)
    .mockReturnValueOnce(fresh.promise);
  const first = s.controller.lookupResult();
  await s.controller.start();
  s.fail();
  const second = s.controller.lookupResult();
  const latest = {
    ...personalResult(),
    own: { ...personalResult().own, opened_safe: 12 },
  };
  fresh.resolve(latest);
  await second;
  old.resolve(personalResult());
  await first;
  expect(s.controller.read().personalResult).toEqual(latest);
  expect(vi.mocked(s.port.result).mock.calls[0]![2].aborted).toBe(true);
  s.controller.dispose();
});
test("offline removes an already published result; wrong-match replies fail closed; unauthorized clears all match memory", async () => {
  const s = await setup();
  s.fail();
  vi.mocked(s.port.result).mockResolvedValueOnce({
    ...personalResult(),
    match_id: "22222222-2222-4222-8222-222222222222",
  });
  await s.controller.lookupResult();
  expect(s.controller.read().personalResult).toBeNull();
  expect(s.controller.read().resultLookup).toBe("unavailable");
  await s.controller.lookupResult();
  expect(s.controller.read().personalResult).toEqual(personalResult());
  s.suspend();
  expect(s.controller.read()).toMatchObject({
    personalResult: null,
    view: null,
    lobby: null,
  });
  s.controller.dispose();
  const t = await setup();
  t.fail();
  vi.mocked(t.port.result).mockRejectedValueOnce(
    new OnlineFailure("auth_required"),
  );
  await t.controller.lookupResult();
  expect(t.controller.read()).toMatchObject({
    personalResult: null,
    view: null,
    lobby: null,
  });
  t.controller.dispose();
});
