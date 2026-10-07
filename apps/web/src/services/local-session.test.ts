import { expect, test, vi } from "vitest";
import {
  LocalController,
  localSeed,
  type SessionClock,
  type SessionPort,
} from "./local-session";
import type { LocalInput } from "@liar/protocol";
import { createActivity } from "./activity";
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}
function setup() {
  let time = 10;
  let tick = () => {};
  const cancel = vi.fn();
  const clock: SessionClock = {
    now: () => time,
    repeat: (cb) => {
      tick = cb;
      return cancel;
    },
  };
  const calls: Array<{ input?: LocalInput; time: number }> = [];
  const core: SessionPort<number> = {
    init: async () => 0,
    advance: async (t) => {
      calls.push({ time: t });
      return 1;
    },
    step: async (input, t) => {
      calls.push({ input, time: t });
      return {
        ack: {
          command_id: input.command_id,
          revision: 1,
          status: "applied",
          error: null,
          duplicate: false,
        },
        view: 2,
      };
    },
    dispose: vi.fn(),
  };
  const controller = new LocalController(
    async () => core,
    clock,
    (view) => view === 9,
  );
  return {
    controller,
    core,
    clock,
    calls,
    cancel,
    tick: () => tick(),
    time: (next: number) => {
      time = next;
    },
  };
}
test("serializes local commands with monotonic time, preserves retries and cancels the timer", async () => {
  const s = setup();
  const changed = vi.fn();
  const unsubscribe = s.controller.subscribe(changed);
  await s.controller.start();
  expect(s.controller.read()).toEqual({
    status: "ready",
    view: 0,
    error: null,
  });
  s.time(20);
  await s.controller.submit({ type: "open", cell: 3 });
  s.time(25);
  await s.controller.resendLast();
  expect(s.calls[0]).toEqual({
    time: 10,
    input: {
      v: 1,
      command_id: 1,
      client_seq: 1,
      action: { type: "open", cell: 3 },
    },
  });
  expect(s.calls[1].input).toEqual(s.calls[0].input);
  s.tick();
  s.tick();
  await s.controller.submit({ type: "attack" });
  expect(s.calls.filter((call) => !call.input)).toHaveLength(1);
  expect(s.calls.at(-1)?.input?.client_seq).toBe(2);
  expect(changed).toHaveBeenCalled();
  unsubscribe();
  s.controller.dispose();
  s.controller.dispose();
  expect(s.cancel).toHaveBeenCalledTimes(1);
  expect(s.core.dispose).toHaveBeenCalledTimes(1);
});
test("disposes a late core and does not publish after the page is gone", async () => {
  const s = setup();
  const arriving = deferred<SessionPort<number>>();
  const controller = new LocalController(
    () => arriving.promise,
    s.clock,
    () => false,
  );
  const listener = vi.fn();
  controller.subscribe(listener);
  const start = controller.start();
  controller.dispose();
  arriving.resolve(s.core);
  await start;
  expect(s.core.dispose).toHaveBeenCalledTimes(1);
  expect(listener).not.toHaveBeenCalled();
});
test("stops terminal games without applying queued commands", async () => {
  const s = setup();
  s.core.advance = async () => 9;
  await s.controller.start();
  s.tick();
  await s.controller.submit({ type: "attack" });
  expect(s.controller.read().view).toBe(9);
  expect(s.cancel).toHaveBeenCalledTimes(1);
  expect(s.calls).toHaveLength(0);
  s.controller.dispose();
});
test("coalesces ticks while work is blocked, bounds inputs and uses processing time in FIFO order", async () => {
  const s = setup(),
    blocked = deferred<number>();
  const times: number[] = [];
  s.core.advance = async (time) => {
    times.push(time);
    return blocked.promise;
  };
  await s.controller.start();
  s.time(40);
  s.tick();
  await Promise.resolve();
  for (let index = 0; index < 100; index++) s.tick();
  const pending = Array.from({ length: 64 }, () =>
    s.controller.submit({ type: "open", cell: 3 }),
  );
  expect(s.controller.read().error).toBe("busy");
  s.time(80);
  blocked.resolve(1);
  await Promise.all(pending);
  expect(times).toEqual([30]);
  expect(s.calls).toHaveLength(63);
  expect(s.calls.every((call) => call.time === 70)).toBe(true);
  expect(s.calls.at(-1)?.input?.client_seq).toBe(63);
  s.controller.dispose();
});
test("domain rejection retains an authoritative view but invalid clock stops the worker safely", async () => {
  const s = setup();
  s.core.step = async (input) => ({
    view: 4,
    ack: {
      command_id: input.command_id,
      revision: 1,
      duplicate: false,
      status: "rejected",
      error: "stunned",
    },
  });
  await s.controller.resendLast();
  await s.controller.submit({ type: "attack" });
  await s.controller.start();
  await s.controller.start();
  await s.controller.submit({ type: "attack" });
  expect(s.controller.read()).toEqual({
    status: "ready",
    view: 4,
    error: "stunned",
  });
  s.time(0);
  await s.controller.submit({ type: "attack" });
  expect(s.controller.read().status).toBe("error");
  expect(s.controller.read().error).toBe("invalid_time");
  expect(s.core.dispose).toHaveBeenCalledTimes(1);
  s.controller.dispose();
  expect(s.core.dispose).toHaveBeenCalledTimes(1);
});
test("init failures hide private details and disposal during initialization never starts a timer", async () => {
  const s = setup();
  const pending = deferred<number>();
  s.core.init = () => pending.promise;
  const starting = s.controller.start();
  await Promise.resolve();
  s.controller.dispose();
  pending.resolve(0);
  await starting;
  expect(s.cancel).not.toHaveBeenCalled();
  expect(s.core.dispose).toHaveBeenCalledTimes(1);
  const failed = new LocalController<number>(
    async () => {
      throw Error("secret failure");
    },
    s.clock,
    () => false,
  );
  await failed.start();
  expect(failed.read().error).toBe("unavailable");
  failed.dispose();
});
test("uses a bounded local random seed without a server account", () => {
  expect(localSeed()).toMatch(/^\d{1,20}$/);
});
test("an update prepare lock blocks a new core before its async factory runs", async () => {
  const s = setup(),
    activity = createActivity();
  const factory = vi.fn(async () => s.core);
  const controller = new LocalController(factory, s.clock, () => false);
  expect(activity.prepare("update")).toBe(true);
  await controller.start(() => {
    activity.hold();
  });
  expect(factory).not.toHaveBeenCalled();
  expect(controller.read().status).toBe("error");
  activity.release("update");
  controller.dispose();
});
