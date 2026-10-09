import { afterEach, expect, test, vi } from "vitest";
import { BoundedReconnect } from "./reconnect";
afterEach(() => vi.useRealTimers());

test("a delayed completion after the monotonic budget expires cannot be accepted even before overdue browser timers run", async () => {
  vi.useFakeTimers();
  let now = 0,
    finish!: (success: boolean) => void;
  const exhausted = vi.fn();
  const loop = new BoundedReconnect(
    () => now,
    () => 0,
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
    exhausted,
  );
  loop.start();
  await vi.advanceTimersByTimeAsync(500);
  now = 30_000;
  finish(true);
  await Promise.resolve();
  await Promise.resolve();
  expect(exhausted).toHaveBeenCalledTimes(1);
  expect(loop.remaining()).toBeNull();
  expect(vi.getTimerCount()).toBe(0);
});

test("backoff stays capped, respects jitter, and one nonsettling attempt cannot fan out before the total budget ends", async () => {
  vi.useFakeTimers();
  const attempt = vi.fn(async () => false),
    exhausted = vi.fn();
  const loop = new BoundedReconnect(
    () => Date.now(),
    () => 1,
    attempt,
    exhausted,
  );
  loop.start();
  for (const delay of [625, 1250, 2500, 5000, 10000]) {
    const previous = attempt.mock.calls.length;
    await vi.advanceTimersByTimeAsync(delay - 1);
    expect(attempt).toHaveBeenCalledTimes(previous);
    await vi.advanceTimersByTimeAsync(1);
    expect(attempt).toHaveBeenCalledTimes(previous + 1);
  }
  await vi.advanceTimersByTimeAsync(10625);
  expect(exhausted).toHaveBeenCalledTimes(1);
  expect(attempt).toHaveBeenCalledTimes(6);
  expect(vi.getTimerCount()).toBe(0);

  let signal!: AbortSignal;
  const stalled = vi.fn((value: AbortSignal) => {
    signal = value;
    return new Promise<boolean>(() => {});
  });
  const blocked = new BoundedReconnect(
    () => Date.now(),
    () => 0,
    stalled,
    exhausted,
  );
  blocked.start();
  await vi.advanceTimersByTimeAsync(10500);
  expect(signal.aborted).toBe(true);
  expect(stalled).toHaveBeenCalledTimes(1);
  await vi.advanceTimersByTimeAsync(19500);
  expect(stalled).toHaveBeenCalledTimes(1);
  expect(exhausted).toHaveBeenCalledTimes(2);
  expect(vi.getTimerCount()).toBe(0);
});

test("acceptance releases retry timers while the established connection's signal stays live", async () => {
  vi.useFakeTimers();
  let signal!: AbortSignal;
  const exhausted = vi.fn();
  const loop = new BoundedReconnect(
    () => Date.now(),
    () => 0,
    async (value) => {
      signal = value;
      return true;
    },
    exhausted,
  );
  loop.start();
  await vi.advanceTimersByTimeAsync(500);
  expect(loop.remaining()).toBeNull();
  expect(signal.aborted).toBe(false);
  await vi.advanceTimersByTimeAsync(60000);
  expect(exhausted).not.toHaveBeenCalled();
  expect(signal.aborted).toBe(false);
  expect(vi.getTimerCount()).toBe(0);
});
