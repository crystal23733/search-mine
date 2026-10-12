// @vitest-environment node
import { expect, test, vi } from "vitest";
import type { DailyRankingsResponse } from "@liar/protocol";
import { DailyRankingsController } from "./rankings";
const empty: DailyRankingsResponse = {
  v: 1,
  metadata: null,
  time: { entries: [], has_more: false },
  completion: { entries: [], has_more: false },
};
function deferred() {
  let resolve!: (value: DailyRankingsResponse) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<DailyRankingsResponse>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { resolve, reject, promise };
}
test("a replaced or disposed public read cannot publish late success or failure", async () => {
  const first = deferred(),
    second = deferred(),
    third = deferred();
  const port = vi
    .fn()
    .mockReturnValueOnce(first.promise)
    .mockReturnValueOnce(second.promise)
    .mockReturnValueOnce(third.promise);
  const c = new DailyRankingsController(port, () =>
    Date.parse("2026-10-07T23:59:59Z"),
  );
  const notify = vi.fn(),
    unsubscribe = c.subscribe(notify);
  const a = c.setOnline(true),
    b = c.load();
  expect(port.mock.calls[0][2].aborted).toBe(true);
  second.resolve(empty);
  await b;
  first.reject(Error("late"));
  await a;
  expect(c.read()).toMatchObject({
    status: "ready",
    date: "2026-10-07",
    result: empty,
  });
  const d = c.load();
  c.dispose();
  expect(port.mock.calls[2][2].aborted).toBe(true);
  const count = notify.mock.calls.length;
  third.resolve(empty);
  await d;
  await c.load();
  await c.setOnline(false);
  expect(port).toHaveBeenCalledTimes(3);
  expect(notify).toHaveBeenCalledTimes(count);
  unsubscribe();
});
test("UTC refresh clears previous data and invalid wall time never sends a request", async () => {
  let wall = Date.parse("2026-10-07T23:59:59Z");
  const next = deferred();
  const port = vi
    .fn()
    .mockResolvedValueOnce(empty)
    .mockReturnValueOnce(next.promise);
  const c = new DailyRankingsController(port, () => wall);
  await c.setOnline(true);
  wall += 1000;
  const pending = c.load();
  expect(c.read()).toEqual({
    status: "loading",
    date: "2026-10-08",
    result: null,
  });
  expect(port.mock.calls[1].slice(0, 2)).toEqual(["2026-10-08", 1]);
  next.resolve(empty);
  await pending;
  wall = NaN;
  await c.load();
  expect(c.read()).toEqual({ status: "error", date: "", result: null });
  expect(port).toHaveBeenCalledTimes(2);
  c.dispose();
});
test("offline cancels and clears data; explicit reads are blocked until reconnect", async () => {
  const old = deferred(),
    recovery = deferred();
  const port = vi
    .fn()
    .mockReturnValueOnce(old.promise)
    .mockReturnValueOnce(recovery.promise);
  const c = new DailyRankingsController(port, () => Date.parse("2026-10-07"));
  const a = c.setOnline(true);
  await c.setOnline(false);
  await c.load();
  expect(port.mock.calls[0][2].aborted).toBe(true);
  expect(c.read().status).toBe("offline");
  old.resolve(empty);
  await a;
  expect(c.read().status).toBe("offline");
  expect(port).toHaveBeenCalledTimes(1);
  const b = c.setOnline(true);
  recovery.resolve(empty);
  await b;
  await c.setOnline(true);
  expect(c.read().status).toBe("ready");
  expect(port).toHaveBeenCalledTimes(2);
  c.dispose();
});
test("a current failed request is an error, and retry can return a normal unpublished result", async () => {
  const port = vi
    .fn()
    .mockRejectedValueOnce(Error("503"))
    .mockResolvedValueOnce(empty);
  const c = new DailyRankingsController(port, () => Date.parse("2026-10-07"));
  await c.setOnline(true);
  expect(c.read()).toMatchObject({ status: "error", result: null });
  await c.load();
  expect(c.read()).toMatchObject({ status: "ready", result: empty });
  c.dispose();
  c.dispose();
});
