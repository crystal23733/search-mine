import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() =>
    localStorage.setItem("liar.tutorial.v1", '"skipped"'),
  );
});
const fixture = JSON.parse(
  readFileSync(
    new URL("../fixtures/native-wasm.json", import.meta.url),
    "utf8",
  ),
) as {
  cases: Array<{
    seed: string;
    difficulty: string;
    initial: unknown;
    events: Array<{
      kind: string;
      input: string;
      time_ms: number;
      expected: unknown;
    }>;
  }>;
};
for (const scenario of fixture.cases) {
  test(`native/WASM parity seed ${scenario.seed}`, async ({ page }) => {
    await page.goto("/");
    const actual = await page.evaluate(async (scenario) => {
      const uri = "/core/liar_wasm.js";
      const wasm = await import(/* @vite-ignore */ uri);
      await wasm.default();
      const session = new wasm.LocalSession(scenario.seed, scenario.difficulty);
      const initial = JSON.parse(session.snapshot());
      const events = scenario.events.map((event) => {
        try {
          return {
            ok: JSON.parse(
              event.kind === "step"
                ? session.step(event.input, event.time_ms)
                : session.advance(event.time_ms),
            ),
          };
        } catch (error) {
          return {
            error: error instanceof Error ? error.message : "unavailable",
          };
        }
      });
      session.free();
      return { initial, events };
    }, scenario);
    expect(actual.initial).toEqual(scenario.initial);
    expect(actual.events).toEqual(
      scenario.events.map((event) => event.expected),
    );
  });
}
test("production worker shares the native projection and terminates cleanly", async ({
  page,
}) => {
  await page.goto("/");
  const actual = await page.evaluate(async () => {
    const uri = "/src/services/core.ts";
    const { createPracticeCore } = await import(/* @vite-ignore */ uri);
    const core = createPracticeCore();
    const initial = await core.init("42", "normal");
    const view = await core.advance(3000);
    let invalid = "";
    try {
      await core.advance(-1);
    } catch (error) {
      invalid = error instanceof Error ? error.message : "";
    }
    core.dispose();
    let disposed = "";
    try {
      await core.snapshot();
    } catch (error) {
      disposed = error instanceof Error ? error.message : "";
    }
    return { initial, view, invalid, disposed };
  });
  expect(actual.initial).toEqual(
    fixture.cases.find((c) => c.seed === "42")!.initial,
  );
  expect(actual.view.phase).toBe("playing");
  expect(actual.view.opponent.opened_safe).toBeGreaterThan(0);
  expect(actual.invalid).toBe("invalid_time");
  expect(actual.disposed).toBe("disposed");
});

test("WASM numeric clock rejects truncation without changing the session", async ({
  page,
}) => {
  await page.goto("/");
  const actual = await page.evaluate(async () => {
    const uri = "/core/liar_wasm.js";
    const wasm = await import(/* @vite-ignore */ uri);
    await wasm.default();
    const session = new wasm.LocalSession("42", "normal");
    const before = session.snapshot();
    const errors = [-1, 1.5, NaN, Infinity, 2 ** 32].map((time) => {
      try {
        session.advance(time);
        return "";
      } catch (error) {
        return error instanceof Error ? error.message : "";
      }
    });
    const unchanged = session.snapshot() === before;
    session.free();
    return { errors, unchanged };
  });
  expect(actual.errors).toEqual(Array(5).fill("invalid_time"));
  expect(actual.unchanged).toBe(true);
});
