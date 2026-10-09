// @vitest-environment node
import { expect, test, vi } from "vitest";
import type { AuthenticatedRequests } from "../auth/requests";
import { createOnlineSocket } from "./socket";
import { matchId, view } from "../../../test/online-fixture";
class Socket extends EventTarget {
  readyState = 1;
  close = vi.fn(() => {
    this.readyState = 3;
  });
  send = vi.fn();
  emit(data: unknown) {
    this.dispatchEvent(
      new MessageEvent("message", {
        data: typeof data === "string" ? data : JSON.stringify(data),
      }),
    );
  }
}
const owner = { accountId: matchId, revision: 1 };
const event = (seq = 1) => ({
  v: 1,
  match_id: matchId,
  server_seq: seq,
  server_time_ms: 0,
  payload: {
    type: "snapshot",
    last_client_seq: 0,
    session_epoch: 2,
    view: view(),
  },
});
const auth: AuthenticatedRequests = {
  execute: async (_o, work, signal) =>
    work({ csrf: "never-in-url", signal: signal! }),
};
test("opens fixed wss URL, buffers until both authority checks finish, then drains snapshot once", async () => {
  const socket = new Socket(),
    factory = vi.fn(() => socket as unknown as WebSocket);
  let post!: () => void;
  const requests: AuthenticatedRequests = {
    execute: async (o, work, s) => {
      expect(o).toEqual(owner);
      const result = await work({ csrf: "proof", signal: s! });
      await new Promise<void>((r) => {
        post = r;
      });
      return result;
    },
  };
  const pending = createOnlineSocket(requests, "https://example.test", factory)(
    owner,
    matchId,
    new AbortController().signal,
  );
  await vi.waitFor(() =>
    expect(factory).toHaveBeenCalledWith("wss://example.test/api/v1/ws"),
  );
  socket.emit(event());
  await vi.waitFor(() => expect(post).toBeTypeOf("function"));
  const received = vi.fn();
  post();
  const connection = await pending;
  expect(received).not.toHaveBeenCalled();
  connection.listen(received, vi.fn());
  expect(received).toHaveBeenCalledExactlyOnceWith(event());
  socket.emit({ ...event(2), payload: { type: "delta", view: view() } });
  expect(received).toHaveBeenCalledTimes(2);
  connection.close();
  expect(socket.close).toHaveBeenCalledTimes(1);
});
test("closes on cancellation, wrong assignment, binary/oversize frames and bounded handshake overflow", async () => {
  for (const data of [
    { ...event(), match_id: "22222222-2222-4222-8222-222222222222" },
    new ArrayBuffer(1),
    " ".repeat(131073),
  ]) {
    const socket = new Socket();
    const pending = createOnlineSocket(
      auth,
      "https://example.test",
      () => socket as unknown as WebSocket,
    )(owner, matchId, new AbortController().signal);
    const rejected = expect(pending).rejects.toThrow();
    await Promise.resolve();
    if (data instanceof ArrayBuffer)
      socket.dispatchEvent(new MessageEvent("message", { data }));
    else socket.emit(data);
    await rejected;
    expect(socket.close).toHaveBeenCalledTimes(1);
  }
  const socket = new Socket(),
    abort = new AbortController();
  const pending = createOnlineSocket(
    auth,
    "https://example.test",
    () => socket as unknown as WebSocket,
  )(owner, matchId, abort.signal);
  const rejected = expect(pending).rejects.toThrow();
  await Promise.resolve();
  abort.abort();
  await rejected;
  expect(socket.close).toHaveBeenCalledTimes(1);
});
test("bounds pre-acceptance buffering and closes when post-snapshot authority verification fails", async () => {
  const socket = new Socket();
  let post!: () => void;
  const requests: AuthenticatedRequests = {
    execute: async (_o, work, s) => {
      const result = await work({ csrf: "proof", signal: s! });
      await new Promise<void>((r) => {
        post = r;
      });
      return result;
    },
  };
  const pending = createOnlineSocket(
    requests,
    "https://example.test",
    () => socket as unknown as WebSocket,
  )(owner, matchId, new AbortController().signal);
  const rejected = expect(pending).rejects.toMatchObject({ code: "malformed" });
  socket.emit(event());
  await vi.waitFor(() => expect(post).toBeTypeOf("function"));
  for (let i = 2; i <= 17; i++)
    socket.emit({ ...event(i), payload: { type: "delta", view: view() } });
  post();
  await rejected;
  expect(socket.close).toHaveBeenCalledTimes(1);
  const second = new Socket();
  const revoked: AuthenticatedRequests = {
    execute: async (_o, work, s) => {
      await work({ csrf: "proof", signal: s! });
      throw Error("post proof failed");
    },
  };
  const next = createOnlineSocket(
    revoked,
    "https://example.test",
    () => second as unknown as WebSocket,
  )(owner, matchId, new AbortController().signal);
  const failed = expect(next).rejects.toThrow("post proof failed");
  second.emit(event());
  await failed;
  expect(second.close).toHaveBeenCalledTimes(1);
});

test("treats a policy close without a public error as revoked authority even if bootstrap would still accept the session", async () => {
  const socket = new Socket();
  const pending = createOnlineSocket(
    auth,
    "https://example.test",
    () => socket as unknown as WebSocket,
  )(owner, matchId, new AbortController().signal);
  socket.emit(event());
  const connection = await pending;
  const ended = vi.fn();
  connection.listen(vi.fn(), ended);
  const closed = new Event("close");
  Object.defineProperty(closed, "code", { value: 1008 });
  socket.dispatchEvent(closed);
  expect(ended).toHaveBeenCalledExactlyOnceWith(
    expect.objectContaining({ code: "unauthorized" }),
  );
});

test("keeps a preceding public error and ordinary network closure distinct from authority revocation", async () => {
  for (const [code, publicError] of [
    [1000, null],
    [1006, null],
    [1008, "rate_limited"],
    [1008, "malformed"],
  ] as const) {
    const socket = new Socket();
    const pending = createOnlineSocket(
      auth,
      "https://example.test",
      () => socket as unknown as WebSocket,
    )(owner, matchId, new AbortController().signal);
    socket.emit(event());
    const connection = await pending;
    const ended = vi.fn();
    const received = vi.fn();
    connection.listen((event) => {
      received(event);
      if (event.payload.type === "error") connection.close();
    }, ended);
    if (publicError)
      socket.emit({
        ...event(2),
        payload: { type: "error", code: publicError },
      });
    const closed = new Event("close");
    Object.defineProperty(closed, "code", { value: code });
    socket.dispatchEvent(closed);
    if (publicError) {
      expect(received).toHaveBeenLastCalledWith(
        expect.objectContaining({
          payload: { type: "error", code: publicError },
        }),
      );
      expect(ended).not.toHaveBeenCalled();
    } else
      expect(ended).toHaveBeenCalledExactlyOnceWith(
        expect.objectContaining({ code: "disconnected" }),
      );
  }
});
