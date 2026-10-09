import { afterEach, expect, test, vi } from "vitest";
import type { LobbyResponse, OnlineEvent, OnlineInput } from "@liar/protocol";
import { OnlineController, type OnlineAuth } from "./controller";
import { OnlineFailure, type OnlineConnection, type OnlinePort } from "./types";
import { matchId, view } from "../../../test/online-fixture";
afterEach(() => vi.useRealTimers());
function deferred<T>() {
  let resolve!: (v: T) => void;
  return {
    promise: new Promise<T>((r) => {
      resolve = r;
    }),
    resolve: (v: T) => resolve(v),
  };
}
const idle: LobbyResponse = {
  v: 1,
  server_time_ms: 0,
  state: { type: "idle" },
};
const queued: LobbyResponse = {
  v: 1,
  server_time_ms: 0,
  state: {
    type: "queued",
    queue_id: matchId,
    deadline_ms: 10000,
    difficulty: "hard",
  },
};
const matched: LobbyResponse = {
  v: 1,
  server_time_ms: 0,
  state: { type: "matched", match_id: matchId, own_seat: 0, opponent: "human" },
};
const room: LobbyResponse = {
  v: 1,
  server_time_ms: 1000,
  state: {
    type: "room",
    room_id: matchId,
    code: "ABCD2345",
    own_seat: 0,
    occupied: [true, false],
    ready: [false, false],
    expires_at_ms: 601000,
  },
};
test("room create and ready abort old polls, use the current room identity and discard late state before exact cancel", async () => {
  vi.useFakeTimers();
  const s = setup();
  await s.controller.start();
  vi.mocked(s.port.lobby).mockResolvedValueOnce(room);
  await s.controller.createRoom();
  expect(s.port.lobby).toHaveBeenLastCalledWith(
    expect.anything(),
    { type: "room_create" },
    expect.any(AbortSignal),
  );
  const late = deferred<LobbyResponse>();
  vi.mocked(s.port.lobby).mockReturnValueOnce(late.promise);
  await vi.advanceTimersByTimeAsync(500);
  const oldSignal = vi.mocked(s.port.lobby).mock.calls.at(-1)![2];
  const accepted: LobbyResponse = {
    ...room,
    state: {
      ...(room.state as Extract<LobbyResponse["state"], { type: "room" }>),
      ready: [true, false],
    },
  };
  vi.mocked(s.port.lobby).mockResolvedValueOnce(accepted);
  await s.controller.setReady(true);
  expect(oldSignal.aborted).toBe(true);
  expect(s.port.lobby).toHaveBeenLastCalledWith(
    expect.anything(),
    { type: "ready", room_id: matchId, ready: true },
    expect.any(AbortSignal),
  );
  late.resolve(room);
  await Promise.resolve();
  expect(s.controller.read().lobby).toEqual(accepted);
  vi.mocked(s.port.lobby).mockResolvedValueOnce(idle);
  await s.controller.cancel();
  expect(s.port.lobby).toHaveBeenLastCalledWith(
    expect.anything(),
    { type: "cancel", identity: { kind: "room", room_id: matchId } },
    expect.any(AbortSignal),
  );
  expect(s.controller.read().status).toBe("ready");
  s.controller.dispose();
});
test("room join canonicalizes input, rejects invalid codes without requests, and ready remains server-owned with duplicate click suppression", async () => {
  const s = setup();
  await s.controller.start();
  await s.controller.joinRoom("ABCD2340");
  expect(s.port.lobby).toHaveBeenCalledTimes(1);
  const joined: LobbyResponse = {
    ...room,
    state: {
      ...(room.state as Extract<LobbyResponse["state"], { type: "room" }>),
      own_seat: 1,
      occupied: [true, true],
      ready: [true, false],
    },
  };
  vi.mocked(s.port.lobby).mockResolvedValueOnce(joined);
  await s.controller.joinRoom(" abcd2345 ");
  expect(s.port.lobby).toHaveBeenLastCalledWith(
    expect.anything(),
    { type: "room_join", code: "ABCD2345" },
    expect.any(AbortSignal),
  );
  const pending = deferred<LobbyResponse>();
  vi.mocked(s.port.lobby).mockReturnValueOnce(pending.promise);
  const ready = s.controller.setReady(true);
  await s.controller.setReady(false);
  expect(s.controller.read().lobby).toEqual(joined);
  expect(s.port.lobby).toHaveBeenCalledTimes(3);
  s.replace();
  pending.resolve(joined);
  await ready;
  expect(s.controller.read().lobby).toBeNull();
  expect(s.controller.read().view).toBeNull();
  s.controller.dispose();
});
test("displaying zero room time never expires an active server room or starts a match while a bounded status is pending", async () => {
  vi.useFakeTimers();
  const s = setup();
  vi.mocked(s.port.lobby).mockResolvedValueOnce(room);
  await s.controller.start();
  expect(s.controller.read().roomMs).toBe(600000);
  const pending = deferred<LobbyResponse>();
  vi.mocked(s.port.lobby).mockReturnValueOnce(pending.promise);
  await vi.advanceTimersByTimeAsync(600000);
  expect(s.controller.read().roomMs).toBe(0);
  expect(s.controller.read().status).toBe("waiting");
  expect(s.controller.read().lobby).toEqual(room);
  expect(s.port.lobby).toHaveBeenCalledTimes(2);
  expect(s.port.connect).not.toHaveBeenCalled();
  pending.resolve(idle);
  await Promise.resolve();
  expect(s.controller.read().status).toBe("ready");
  s.controller.dispose();
});
function setup() {
  let account: string | null = matchId,
    revision = 1,
    listener = () => {};
  const auth: OnlineAuth = {
    execute: async (_owner, work, signal) =>
      work({ csrf: "proof", signal: signal ?? new AbortController().signal }),
    account: () => account,
    connected: () => account !== null,
    revision: () => revision,
    subscribe: (fn) => {
      listener = fn;
      return () => {};
    },
    invalidate: () => {
      account = null;
      revision++;
      listener();
    },
  };
  let event: (v: OnlineEvent) => void = () => {},
    closed: (e: OnlineFailure) => void = () => {};
  const connection: OnlineConnection = {
    listen: (e, c) => {
      event = e;
      closed = c;
    },
    send: vi.fn((_input: OnlineInput) => true),
    close: vi.fn(),
  };
  const port: OnlinePort = {
    lobby: vi.fn(async () => idle),
    connect: vi.fn(async () => connection),
  };
  const controller = new OnlineController(auth, port, () => Date.now());
  let seq = 0;
  return {
    controller,
    port,
    connection,
    auth,
    replace: () => {
      account = "22222222-2222-4222-8222-222222222222";
      revision++;
      listener();
    },
    event: (payload: OnlineEvent["payload"], sequence = ++seq) =>
      event({
        v: 1,
        match_id: matchId,
        server_seq: sequence,
        server_time_ms: 0,
        payload,
      }),
    closed: (code = "disconnected") => closed(new OnlineFailure(code)),
  };
}
test("polls one request at a time only after waiting reply, then cancels exact identity and ignores late status", async () => {
  vi.useFakeTimers();
  const s = setup();
  await s.controller.start();
  expect(s.controller.read().status).toBe("ready");
  vi.mocked(s.port.lobby).mockResolvedValueOnce(queued);
  await s.controller.join("hard");
  expect(s.controller.read().status).toBe("waiting");
  const late = deferred<LobbyResponse>();
  vi.mocked(s.port.lobby).mockReturnValueOnce(late.promise);
  await vi.advanceTimersByTimeAsync(500);
  expect(s.port.lobby).toHaveBeenCalledTimes(3);
  await vi.advanceTimersByTimeAsync(5000);
  expect(s.port.lobby).toHaveBeenCalledTimes(3);
  vi.mocked(s.port.lobby).mockResolvedValueOnce(idle);
  await s.controller.cancel();
  expect(s.port.lobby).toHaveBeenLastCalledWith(
    expect.anything(),
    { type: "cancel", identity: { kind: "queue", queue_id: matchId } },
    expect.any(AbortSignal),
  );
  late.resolve(matched);
  await Promise.resolve();
  expect(s.controller.read().status).toBe("ready");
  expect(s.port.connect).not.toHaveBeenCalled();
  s.controller.dispose();
  expect(vi.getTimerCount()).toBe(0);
});
test("uses server epoch/revision, bounded command IDs and ACKs without optimistic board or own clock fields", async () => {
  const s = setup();
  vi.mocked(s.port.lobby).mockResolvedValueOnce(matched);
  await s.controller.start();
  s.event({
    type: "snapshot",
    session_epoch: 4,
    view: { ...view(), phase: "playing", revision: 7 },
  });
  expect(s.controller.read().status).toBe("playing");
  s.controller.submit({ type: "open", cell: 2 });
  const input = vi.mocked(s.connection.send).mock.calls[0][0];
  expect(input).toEqual({
    v: 1,
    match_id: matchId,
    command_id: expect.stringMatching(/^[a-f0-9-]{36}$/),
    client_seq: 1,
    session_epoch: 4,
    known_revision: 7,
    action: { type: "open", cell: 2 },
  });
  expect(s.controller.read().view?.revision).toBe(7);
  expect(s.controller.read().pending).toBe(1);
  s.event({
    type: "ack",
    command_id: input.command_id,
    revision: 8,
    status: "applied",
    error: null,
    duplicate: false,
  });
  expect(s.controller.read().pending).toBe(0);
  expect(s.controller.read().view?.revision).toBe(7);
  s.event({
    type: "delta",
    view: { ...view(), phase: "playing", revision: 8 },
  });
  expect(s.controller.read().view?.revision).toBe(8);
  s.controller.dispose();
});
test("clears previous public view and closes socket when account changes; late callbacks cannot restore it", async () => {
  const s = setup();
  vi.mocked(s.port.lobby).mockResolvedValueOnce(matched);
  await s.controller.start();
  s.event({ type: "snapshot", session_epoch: 1, view: view() });
  expect(s.controller.read().view).not.toBeNull();
  s.replace();
  expect(s.connection.close).toHaveBeenCalled();
  expect(s.controller.read().view).toBeNull();
  s.event({ type: "delta", view: view() });
  expect(s.controller.read().view).toBeNull();
  s.controller.dispose();
});
test("preserves unconfirmed result on close and reports actual recording saved/failed only", async () => {
  const s = setup();
  vi.mocked(s.port.lobby).mockResolvedValueOnce(matched);
  await s.controller.start();
  s.event({ type: "snapshot", session_epoch: 1, view: view() });
  const finished = {
    ...view(),
    revision: 1,
    phase: "finished" as const,
    result: {
      reason: "timeout" as const,
      outcome: "draw" as const,
      completed: true,
    },
  };
  s.event({ type: "match_end", view: finished, recording: "pending" });
  s.closed();
  expect(s.controller.read().recording).toBe("pending");
  expect(s.controller.read().error).toBe("disconnected");
  expect(s.controller.read().view?.result).toEqual(finished.result);
  s.controller.dispose();
});
test("fails closed on stream gaps, rejects input afterward and disposes pending deadline", async () => {
  vi.useFakeTimers();
  const s = setup();
  vi.mocked(s.port.lobby).mockResolvedValueOnce(matched);
  await s.controller.start();
  s.event({
    type: "snapshot",
    session_epoch: 1,
    view: { ...view(), phase: "playing" },
  });
  s.controller.submit({ type: "flag", cell: 2 });
  s.event(
    { type: "delta", view: { ...view(), phase: "playing", revision: 1 } },
    4,
  );
  expect(s.controller.read().error).toBe("stale");
  s.controller.submit({ type: "attack" });
  expect(s.connection.send).toHaveBeenCalledTimes(1);
  expect(s.connection.close).toHaveBeenCalled();
  s.controller.dispose();
  expect(vi.getTimerCount()).toBe(0);
});
test("caps unacknowledged commands and times out the actual missing ACK without retrying", async () => {
  vi.useFakeTimers();
  const s = setup();
  vi.mocked(s.port.lobby).mockResolvedValueOnce(matched);
  await s.controller.start();
  s.event({
    type: "snapshot",
    session_epoch: 1,
    view: { ...view(), phase: "playing" },
  });
  for (let i = 0; i < 17; i++) s.controller.submit({ type: "flag", cell: i });
  expect(s.connection.send).toHaveBeenCalledTimes(16);
  expect(s.controller.read().error).toBe("capacity");
  await vi.advanceTimersByTimeAsync(10000);
  expect(s.controller.read().error).toBe("timeout");
  expect(s.connection.send).toHaveBeenCalledTimes(16);
  expect(s.controller.read().pending).toBe(0);
  s.controller.dispose();
  expect(vi.getTimerCount()).toBe(0);
});
test("verifies the same authority after socket close and clears a revoked public board", async () => {
  const s = setup();
  vi.mocked(s.port.lobby).mockResolvedValueOnce(matched);
  await s.controller.start();
  s.event({ type: "snapshot", session_epoch: 1, view: view() });
  const proof = vi.spyOn(s.auth, "execute").mockImplementation(async () => {
    s.auth.invalidate();
    throw Error("revoked");
  });
  s.closed();
  await vi.waitFor(() => expect(s.controller.read().view).toBeNull());
  expect(proof).toHaveBeenCalledExactlyOnceWith(
    { accountId: matchId, revision: 1 },
    expect.any(Function),
    expect.any(AbortSignal),
  );
  s.controller.dispose();
});

test("policy closure invalidates immediately without trusting a proof before server deletion commits", async () => {
  const s = setup();
  vi.mocked(s.port.lobby).mockResolvedValueOnce(matched);
  await s.controller.start();
  s.event({ type: "snapshot", session_epoch: 1, view: view() });
  const proof = vi.spyOn(s.auth, "execute");
  s.closed("unauthorized");
  expect(s.auth.account()).toBeNull();
  expect(s.controller.read().view).toBeNull();
  expect(proof).not.toHaveBeenCalled();
  s.controller.dispose();
});
test("does not connect or report cancel success when the reservation committed during cancellation", async () => {
  const s = setup();
  await s.controller.start();
  vi.mocked(s.port.lobby).mockResolvedValueOnce(queued);
  await s.controller.join("hard");
  vi.mocked(s.port.lobby).mockResolvedValueOnce(matched);
  await s.controller.cancel();
  expect(s.controller.read().error).toBe("busy");
  expect(s.controller.read().status).toBe("error");
  expect(s.port.connect).not.toHaveBeenCalled();
  s.controller.dispose();
});
