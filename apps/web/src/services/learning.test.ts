// @vitest-environment node
import { expect, test, vi } from "vitest";
import { createLearning, LEARNING_KEY } from "./learning";
test("persists only tutorial status and distinguishes skip from completion", () => {
  const storage = { getItem: () => null, setItem: vi.fn() };
  const learning = createLearning(storage);
  expect(learning.read()).toBe("new");
  learning.mark("skipped");
  expect(learning.read()).toBe("skipped");
  expect(storage.setItem).toHaveBeenCalledWith(LEARNING_KEY, '"skipped"');
  learning.mark("complete");
  expect(learning.read()).toBe("complete");
  expect(
    createLearning({ ...storage, getItem: () => '"complete"' }).read(),
  ).toBe("complete");
});
test("storage errors remain usable in memory and invalid values never count as completion", () => {
  const storage = {
    getItem: () => {
      throw Error("denied");
    },
    setItem: () => {
      throw Error("denied");
    },
  };
  const learning = createLearning(storage);
  learning.mark("skipped");
  expect(learning.read()).toBe("skipped");
  expect(learning.persistent()).toBe(false);
  for (const value of ["{}", "[]", '"new"', "broken", "x".repeat(2049)])
    expect(
      createLearning({ getItem: () => value, setItem: () => {} }).read(),
    ).toBe("new");
  const memory = createLearning();
  memory.mark("complete");
  expect(memory.read()).toBe("complete");
  expect(memory.persistent()).toBe(false);
});
