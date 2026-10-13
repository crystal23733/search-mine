// @vitest-environment node
import { expect, test, vi } from "vitest";
import type { DailyCore } from "@liar/core-bridge";
import type {
  DailyHeartbeatResponse,
  DailyReplay,
  DailyStartResponse,
  DailySubmitResponse,
} from "@liar/protocol";
import { deferred, officialView } from "../../../test/official-daily";
import { createActivity } from "../activity";
import { createDailyRecords, dailyId } from "../daily-records";
import { OfficialDailyController, type DailyChallengeApi } from "./official";
function setup() {
  let account: string | null = "account-a",
    revision = 1,
    online = true,
    now = 0;
  const listeners = new Set<() => void>(),
    timers = new Map<number, () => void>();
  const auth = {
    account: () => account,
    revision: () => revision,
    connected: () => online,
    subscribe: (cb: () => void) => {
      listeners.add(cb);
      return () => {
        listeners.delete(cb);
      };
    },
  };
  let view = officialView();
  const metadata = view.metadata;
  const replay: DailyReplay = {
    v: 1,
    metadata,
    inputs: [],
    final_time_ms: 8000,
  };
  const core: DailyCore = {
    init: vi.fn(async () => view),
    snapshot: async () => view,
    advance: vi.fn(async () => view),
    step: vi.fn<DailyCore["step"]>(async (input) => {
      view = { ...view, completed: true, phase: "finished", elapsed_ms: 5000 };
      return {
        view,
        ack: {
          command_id: input.command_id,
          revision: 1,
          status: "applied",
          error: null,
          duplicate: false,
        },
      };
    }),
    replay: vi.fn(async () => replay),
    dispose: vi.fn(),
  };
  const grant = (attempt: string): DailyStartResponse => ({
    v: 1,
    account_id: account!,
    attempt_id: attempt,
    ticket_id: "ticket",
    metadata,
    rules: view.rules,
    ttl_ms: 300000,
    heartbeat_interval_ms: 15000,
  });
  const response = (attempt: string, timed = true): DailySubmitResponse => ({
    v: 1,
    account_id: account!,
    attempt_id: attempt,
    disposition: "official",
    trust: timed ? "verified_time" : "verified_completion",
    receipt: {
      completion_id: "completion",
      attempt_id: attempt,
      metadata,
      opened_safe: 216,
      mistakes: 0,
      replayed_elapsed_ms: 5000,
      verified_elapsed_ms: timed ? 6000 : null,
      completed_at: 1791331200,
    },
  });
  const api = {
    start: vi.fn<DailyChallengeApi["start"]>(async (_owner, attempt) =>
      grant(attempt),
    ),
    heartbeat: vi.fn(
      async (
        _owner,
        _ticket,
        sequence: number,
      ): Promise<DailyHeartbeatResponse> => ({
        v: 1,
        account_id: account!,
        ticket_id: "ticket",
        ack: { sequence, elapsed_ms: now },
      }),
    ),
    submitTimed: vi.fn<DailyChallengeApi["submitTimed"]>(
      async (_owner, attempt, ticket) => response(attempt, ticket !== null),
    ),
    definition: vi.fn(async () => ({ v: 1, metadata, rules: view.rules })),
    own: vi.fn(async () => ({
      v: 1,
      account_id: account!,
      trust: "verified_completion" as const,
      receipt: null as DailySubmitResponse["receipt"] | null,
    })),
  };
  const activity = createActivity(),
    records = createDailyRecords();
  const factory = vi.fn(async () => core),
    wallClock = vi.fn(() => Date.parse("2026-10-07T23:59:59Z"));
  const c = new OfficialDailyController({
    auth,
    api,
    core: factory,
    records,
    activity,
    wallClock,
    clock: {
      now: () => now,
      repeat: (cb, ms) => {
        timers.set(ms, cb);
        return () => {
          timers.delete(ms);
        };
      },
    },
  });
  c.activate();
  const owner = () => ({ accountId: account!, revision });
  const change = (next: string | null, connected = true) => {
    account = next;
    online = connected;
    revision++;
    for (const cb of listeners) cb();
  };
  const pulse = (ms: number) => {
    now += ms;
    timers.get(ms)?.();
  };
  const finish = async () => {
    await c.submit({ type: "open", cell: 1 });
    await vi.waitFor(() =>
      expect(["result", "pending", "error", "interrupted"]).toContain(
        c.read().status,
      ),
    );
  };
  return {
    c,
    api,
    core,
    factory,
    activity,
    records,
    metadata,
    replay,
    grant,
    response,
    owner,
    change,
    pulse,
    timers,
    wallClock,
    finish,
  };
}
test("only an explicit current owner can start; duplicate starts and lost replies retain one attempt", async () => {
  const s = setup();
  await s.c.start({ ...s.owner(), revision: 99 });
  expect(s.api.start).not.toHaveBeenCalled();
  const gate = deferred<DailyStartResponse>();
  s.api.start.mockReturnValueOnce(gate.promise);
  const start = s.c.start(s.owner());
  await s.c.start(s.owner());
  expect(s.api.start).toHaveBeenCalledTimes(1);
  gate.reject(Error("lost"));
  await start;
  const attempt = s.api.start.mock.calls[0][1];
  await s.c.start(s.owner());
  expect(s.api.start.mock.calls[1][1]).toBe(attempt);
  expect(s.core.init).toHaveBeenCalledWith(
    "2026-10-07",
    1,
    s.grant(attempt).rules,
  );
  await s.c.retry();
  await s.c.inspect();
  expect(s.api.own).not.toHaveBeenCalled();
  s.c.dispose();
  s.c.activate();
  await s.c.start(s.owner());
  expect(s.core.dispose).toHaveBeenCalledTimes(1);
  expect(s.timers.size).toBe(0);
  expect(s.activity.read().busy).toBe(false);
});
test("accepted metadata mismatch never publishes a board or submits", async () => {
  const s = setup();
  vi.mocked(s.core.init).mockResolvedValueOnce({
    ...officialView(),
    metadata: { ...s.metadata, date: "2026-10-08" },
  });
  await s.c.start(s.owner());
  expect(s.c.read()).toMatchObject({ status: "error", view: null });
  expect(s.core.dispose).toHaveBeenCalledTimes(1);
  expect(s.api.submitTimed).not.toHaveBeenCalled();
  s.c.dispose();
});
test("a late start and late core factory after account replacement are discarded and disposed", async () => {
  const s = setup(),
    gate = deferred<DailyStartResponse>();
  s.api.start.mockReturnValueOnce(gate.promise);
  const old = s.c.start(s.owner()),
    signal = s.api.start.mock.calls[0][2] as AbortSignal;
  s.change("account-b");
  gate.resolve(s.grant("old"));
  await old;
  expect(signal.aborted).toBe(true);
  expect(s.factory).not.toHaveBeenCalled();
  const worker = deferred<DailyCore>();
  s.factory.mockReturnValueOnce(worker.promise);
  const newer = s.c.start(s.owner());
  await vi.waitFor(() => expect(s.factory).toHaveBeenCalled());
  s.change("account-c");
  worker.resolve(s.core);
  await newer;
  expect(s.core.dispose).toHaveBeenCalledTimes(1);
  expect(s.c.read().view).toBeNull();
  s.c.dispose();
});
test("heartbeat requests never overlap and a null ACK permanently selects completion-only submission", async () => {
  const s = setup(),
    gate = deferred<DailyHeartbeatResponse>();
  await s.c.start(s.owner());
  s.api.heartbeat.mockReturnValueOnce(gate.promise);
  s.pulse(15000);
  s.pulse(15000);
  expect(s.api.heartbeat).toHaveBeenCalledTimes(1);
  expect(s.api.heartbeat.mock.calls[0][2]).toBe(1);
  gate.resolve({
    v: 1,
    account_id: "account-a",
    ticket_id: "ticket",
    ack: null,
  });
  await vi.waitFor(() => expect(s.c.read().timing).toBe(false));
  s.pulse(15000);
  expect(s.api.heartbeat).toHaveBeenCalledTimes(1);
  await s.finish();
  expect(s.api.submitTimed.mock.calls[0][2]).toBeNull();
  expect(s.c.read().trust).toBe("verified_completion");
  expect((await s.records.pending()).value).toHaveLength(0);
  expect((await s.records.list()).value).toHaveLength(1);
  s.c.dispose();
});
test("successful sequential heartbeats retain the ticket and failures do not infer recovery", async () => {
  const s = setup();
  await s.c.start(s.owner());
  s.pulse(15000);
  await vi.waitFor(() => expect(s.api.heartbeat).toHaveBeenCalledTimes(1));
  await Promise.resolve();
  await Promise.resolve();
  s.api.heartbeat.mockRejectedValueOnce(Error("503"));
  s.pulse(15000);
  await vi.waitFor(() => expect(s.c.read().timing).toBe(false));
  expect(s.api.heartbeat.mock.calls[1][2]).toBe(2);
  await s.finish();
  expect(s.c.read().trust).toBe("verified_completion");
  s.c.dispose();
});
test("save precedes submission, a failure preserves its candidate and explicit retry keeps the attempt", async () => {
  const s = setup(),
    saved = deferred<Awaited<ReturnType<typeof s.records.saveFirst>>>();
  const save = s.records.saveFirst.bind(s.records);
  vi.spyOn(s.records, "saveFirst").mockImplementationOnce(async (record) => {
    const result = await save(record);
    await saved.promise;
    return result;
  });
  s.api.submitTimed.mockRejectedValueOnce(Error("503"));
  await s.c.start(s.owner());
  await s.c.submit({ type: "open", cell: 1 });
  await vi.waitFor(() => expect(s.c.read().status).toBe("saving"));
  expect(s.api.submitTimed).not.toHaveBeenCalled();
  saved.resolve({
    value: { record: (await s.records.pending()).value[0], inserted: true },
    warning: false,
  });
  await vi.waitFor(() => expect(s.c.read().status).toBe("pending"));
  const attempt = s.api.submitTimed.mock.calls[0][1];
  expect((await s.records.pending()).value[0].attempt_id).toBe(attempt);
  s.c.setOnline(true);
  await s.c.start(s.owner());
  expect(s.api.submitTimed).toHaveBeenCalledTimes(1);
  const a = s.c.retry(),
    b = s.c.retry();
  await Promise.all([a, b]);
  expect(s.api.submitTimed.mock.calls[1][1]).toBe(attempt);
  expect(s.api.submitTimed).toHaveBeenCalledTimes(2);
  expect((await s.records.pending()).value).toHaveLength(0);
  expect(s.c.result()?.trust).toBe("verified_time");
  s.c.dispose();
});
test("Practice preserves the original first receipt and never removes another local attempt", async () => {
  const s = setup();
  await s.records.saveFirst({
    v: 1,
    id: dailyId(s.metadata),
    status: "unverified",
    attempt_id: "earlier-local",
    metadata: s.metadata,
    elapsed_ms: 5000,
    mistakes: 0,
    replay: s.replay,
  });
  s.api.submitTimed.mockImplementationOnce(async (_owner, attempt) => ({
    ...s.response(attempt),
    disposition: "practice",
    receipt: {
      ...s.response("first-official").receipt,
      verified_elapsed_ms: 12345,
      mistakes: 3,
    },
  }));
  await s.c.start(s.owner());
  await s.finish();
  expect(s.c.read()).toMatchObject({
    status: "result",
    disposition: "practice",
    receipt: {
      attempt_id: "first-official",
      mistakes: 3,
      verified_elapsed_ms: 12345,
    },
  });
  expect((await s.records.pending()).value[0].attempt_id).toBe("earlier-local");
  s.c.dispose();
});
test("account replacement aborts a pending submission and cannot publish its late receipt", async () => {
  const s = setup(),
    submitted = deferred<DailySubmitResponse>();
  s.api.submitTimed.mockReturnValueOnce(submitted.promise);
  await s.c.start(s.owner());
  await s.c.submit({ type: "open", cell: 1 });
  await vi.waitFor(() => expect(s.api.submitTimed).toHaveBeenCalled());
  const signal = s.api.submitTimed.mock.calls[0][4] as AbortSignal;
  s.change("account-b");
  submitted.resolve(s.response("late"));
  await vi.waitFor(() => expect(s.activity.read().busy).toBe(false));
  expect(signal.aborted).toBe(true);
  expect(s.c.result()).toBeNull();
  expect(s.c.read().receipt).toBeNull();
  expect((await s.records.pending()).value).toHaveLength(1);
  s.c.dispose();
});
test("completed replay capture survives disposal as an unverified candidate and cannot submit", async () => {
  const s = setup(),
    replay = deferred<DailyReplay>();
  vi.mocked(s.core.replay).mockReturnValueOnce(replay.promise);
  await s.c.start(s.owner());
  await s.c.submit({ type: "open", cell: 1 });
  s.c.dispose();
  expect(s.core.dispose).not.toHaveBeenCalled();
  replay.resolve(s.replay);
  await vi.waitFor(() => expect(s.core.dispose).toHaveBeenCalledTimes(1));
  expect((await s.records.pending()).value).toHaveLength(1);
  expect(s.api.submitTimed).not.toHaveBeenCalled();
});
test("own absent, failed and verified reads are distinct; a late read cannot release a new run", async () => {
  const s = setup();
  await s.c.inspect();
  expect(s.c.read().status).toBe("empty");
  s.api.own.mockRejectedValueOnce(Error("503"));
  await s.c.inspect();
  expect(s.c.read().status).toBe("error");
  s.api.own.mockResolvedValueOnce({
    v: 1,
    account_id: "account-a",
    trust: "verified_completion",
    receipt: s.response("old", false).receipt,
  });
  await s.c.inspect();
  expect(s.c.result()?.receipt.verified_elapsed_ms).toBeNull();
  const definition = deferred<Awaited<ReturnType<typeof s.api.definition>>>();
  s.api.definition.mockReturnValueOnce(definition.promise);
  const old = s.c.inspect();
  s.change("account-b");
  await s.c.start(s.owner());
  definition.resolve({
    v: 1,
    metadata: s.metadata,
    rules: officialView().rules,
  });
  await old;
  expect(s.c.read().status).toBe("playing");
  expect(s.activity.read().busy).toBe(true);
  s.c.dispose();
});
test("offline interrupts private state and explicit operations stay blocked until restored", async () => {
  const s = setup();
  s.change(null);
  await s.c.inspect();
  expect(s.api.definition).not.toHaveBeenCalled();
  s.change("account-a");
  await s.c.start(s.owner());
  s.c.setOnline(false);
  expect(s.c.read()).toMatchObject({
    status: "interrupted",
    view: null,
    receipt: null,
  });
  expect(s.core.dispose).toHaveBeenCalledTimes(1);
  await s.c.start(s.owner());
  await s.c.inspect();
  expect(s.api.start).toHaveBeenCalledTimes(1);
  s.c.setOnline(true);
  expect(s.api.start).toHaveBeenCalledTimes(1);
  s.c.dispose();
});
test("invalid browser interval and wall time fail closed without a game or own request", async () => {
  const s = setup();
  s.api.start.mockImplementationOnce(async (_owner, attempt) => ({
    ...s.grant(attempt),
    heartbeat_interval_ms: 2147483648,
  }));
  await s.c.start(s.owner());
  expect(s.factory).not.toHaveBeenCalled();
  expect(s.c.read().status).toBe("error");
  s.wallClock.mockReturnValueOnce(NaN);
  await s.c.inspect();
  expect(s.api.definition).not.toHaveBeenCalled();
  s.c.dispose();
});
test("storage failures warn without blocking verification; malformed replay never submits", async () => {
  const s = setup();
  vi.spyOn(s.records, "saveFirst").mockRejectedValueOnce(Error("storage"));
  vi.spyOn(s.records, "removePending").mockRejectedValueOnce(Error("storage"));
  await s.c.start(s.owner());
  await s.finish();
  expect(s.c.read()).toMatchObject({ status: "result", warning: true });
  vi.mocked(s.core.replay).mockResolvedValueOnce({
    ...s.replay,
    metadata: { ...s.metadata, seed: "99" },
  });
  await s.c.start(s.owner());
  await s.finish();
  expect(s.c.read().status).toBe("error");
  expect(s.api.submitTimed).toHaveBeenCalledTimes(1);
  s.c.dispose();
});
test("factory rejection is a bounded error and an unfinished timeout never stores or submits", async () => {
  const s = setup();
  s.factory.mockRejectedValueOnce(Error("worker"));
  await s.c.start(s.owner());
  expect(s.c.read().status).toBe("error");
  await s.c.start(s.owner());
  vi.mocked(s.core.advance).mockResolvedValueOnce({
    ...officialView(),
    phase: "finished",
    completed: false,
  });
  s.pulse(250);
  await vi.waitFor(() => expect(s.c.read().status).toBe("incomplete"));
  expect(s.api.submitTimed).not.toHaveBeenCalled();
  expect((await s.records.pending()).value).toHaveLength(0);
  s.c.dispose();
});
test("completion waits for the in-flight heartbeat before deciding whether to send its ticket", async () => {
  const s = setup(),
    beat = deferred<DailyHeartbeatResponse>();
  await s.c.start(s.owner());
  s.api.heartbeat.mockReturnValueOnce(beat.promise);
  s.pulse(15000);
  await s.c.submit({ type: "open", cell: 1 });
  await vi.waitFor(async () =>
    expect((await s.records.pending()).value).toHaveLength(1),
  );
  expect(s.api.submitTimed).not.toHaveBeenCalled();
  expect(s.timers.size).toBe(0);
  beat.resolve({
    v: 1,
    account_id: "account-a",
    ticket_id: "ticket",
    ack: null,
  });
  await vi.waitFor(() => expect(s.c.read().status).toBe("result"));
  expect(s.api.submitTimed.mock.calls[0][2]).toBeNull();
  s.c.dispose();
});
test("an aborted old heartbeat cannot change the new account run or its sequence", async () => {
  const s = setup(),
    beat = deferred<DailyHeartbeatResponse>();
  await s.c.start(s.owner());
  s.api.heartbeat.mockReturnValueOnce(beat.promise);
  s.pulse(15000);
  s.change("account-b");
  await s.c.start(s.owner());
  beat.resolve({
    v: 1,
    account_id: "account-a",
    ticket_id: "ticket",
    ack: null,
  });
  await Promise.resolve();
  await Promise.resolve();
  expect(s.c.read()).toMatchObject({ status: "playing", timing: true });
  s.pulse(15000);
  expect(s.api.heartbeat.mock.calls[1][2]).toBe(1);
  s.c.dispose();
});
