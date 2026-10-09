// @vitest-environment node
import { expect, test, vi } from "vitest";
import { createActivity } from "./activity";
test("active games and record writes prevent an update; prepared idle tabs cannot start another game", () => {
  const activity = createActivity();
  const first = activity.hold(),
    second = activity.hold();
  expect(activity.prepare("update")).toBe(false);
  first();
  first();
  expect(activity.read().busy).toBe(true);
  second();
  expect(activity.prepare("update")).toBe(true);
  expect(() => activity.hold()).toThrow("updating");
  activity.release("foreign");
  expect(activity.read().locked).toBe(true);
  activity.release("update");
  expect(activity.read()).toEqual({ busy: false, locked: false });
});
test("a lost update coordinator releases the lock after its bounded lease", async () => {
  vi.useFakeTimers();
  try {
    const activity = createActivity();
    const listener = vi.fn();
    const unsubscribe = activity.subscribe(listener);
    activity.prepare("lost");
    expect(activity.prepare("other")).toBe(false);
    await vi.advanceTimersByTimeAsync(15000);
    expect(activity.read().locked).toBe(false);
    const release = activity.hold();
    release();
    expect(listener).toHaveBeenCalled();
    unsubscribe();
  } finally {
    vi.useRealTimers();
  }
});
