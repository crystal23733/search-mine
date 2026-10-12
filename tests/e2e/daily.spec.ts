import { expect, test } from "@playwright/test";
import { readFileSync, mkdirSync } from "node:fs";
import { unrequestedServiceCall } from "./network";
import type {
  DailyView,
  DailyReplay,
  DailyStep,
} from "../../packages/protocol/src/index";
const nativeFixture = JSON.parse(
  readFileSync(
    new URL("../fixtures/native-wasm.json", import.meta.url),
    "utf8",
  ),
);
type DailyFixture = {
  date: string;
  seed_version: number;
  initial: DailyView;
  events: Array<{ input: string; time_ms: number; expected: DailyStep }>;
  replay: DailyReplay;
};
const daily = nativeFixture.daily as DailyFixture;
const acceptedDaily = nativeFixture.accepted_daily as DailyFixture;
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() =>
    localStorage.setItem("liar.tutorial.v1", '"skipped"'),
  );
  await page.clock.setFixedTime(new Date("2026-10-07T23:59:59Z"));
});
test("production daily worker uses the server accepted snapshot for the full completed replay", async ({
  page,
}) => {
  await page.goto("/en/settings");
  const actual = await page.evaluate(async (fixture) => {
    const wasmUri = "/core/liar_wasm.js";
    const wasm = await import(/* @vite-ignore */ wasmUri);
    await wasm.default();
    const native = wasm.DailySession.withRules(
      fixture.date,
      fixture.seed_version,
      JSON.stringify(fixture.initial.rules),
    );
    let wasmResult;
    try {
      wasmResult = {
        initial: JSON.parse(native.snapshot()),
        steps: fixture.events.map((event) =>
          JSON.parse(native.step(event.input, event.time_ms)),
        ),
        replay: JSON.parse(native.replay()),
      };
    } finally {
      native.free();
    }
    const uri = "/src/services/core.ts";
    const { createDailyCore } = await import(/* @vite-ignore */ uri);
    const core = createDailyCore();
    try {
      const initial = await core.init(
        fixture.date,
        fixture.seed_version,
        fixture.initial.rules,
      );
      const steps = [];
      for (const event of fixture.events)
        steps.push(await core.step(JSON.parse(event.input), event.time_ms));
      return { initial, steps, replay: await core.replay(), wasm: wasmResult };
    } finally {
      core.dispose();
    }
  }, acceptedDaily);
  expect(acceptedDaily.initial.rules.rules.countdown_ms).toBe(5000);
  expect(acceptedDaily.initial.metadata.seed).not.toBe(
    daily.initial.metadata.seed,
  );
  expect(actual.initial).toEqual(acceptedDaily.initial);
  expect(actual.steps).toEqual(
    acceptedDaily.events.map((event) => event.expected),
  );
  expect(actual.replay).toEqual(acceptedDaily.replay);
  expect(actual.wasm).toEqual({
    initial: acceptedDaily.initial,
    steps: acceptedDaily.events.map((event) => event.expected),
    replay: acceptedDaily.replay,
  });
});

test("accepted daily rules fail closed in WASM and a rejected worker init can recover without fallback", async ({
  page,
}) => {
  await page.goto("/en/settings");
  const actual = await page.evaluate(async (fixture) => {
    const wasmUri = "/core/liar_wasm.js";
    const wasm = await import(/* @vite-ignore */ wasmUri);
    await wasm.default();
    const rules = fixture.initial.rules;
    const source = JSON.stringify(rules);
    const exact =
      source + " ".repeat(16384 - new TextEncoder().encode(source).length);
    const invalid = [
      "{",
      "null",
      exact + " ",
      "한".repeat(8192),
      source.replace("{", '{"hash":"duplicate",'),
      JSON.stringify({ ...rules, hash: "forged" }),
      JSON.stringify({ ...rules, solver_version: 999 }),
      JSON.stringify({ ...rules, rng_version: 999 }),
      JSON.stringify({ ...rules, extra: true }),
      JSON.stringify({ ...rules, rules: { ...rules.rules, extra: true } }),
      JSON.stringify({ ...rules, rules: { ...rules.rules, countdown_ms: 0 } }),
    ];
    const errors = invalid.map((input) => {
      try {
        const value = wasm.DailySession.withRules(fixture.date, 1, input);
        value.free();
        return "accepted";
      } catch (error) {
        return (error as Error).message;
      }
    });
    const exactSession = wasm.DailySession.withRules(fixture.date, 1, exact);
    const exactView = JSON.parse(exactSession.snapshot());
    exactSession.free();
    const versions = [0, 1.5, 2, 65537, NaN, Infinity].map((version) => {
      try {
        const value = wasm.DailySession.withRules(
          fixture.date,
          version,
          source,
        );
        value.free();
        return "accepted";
      } catch (error) {
        return (error as Error).message;
      }
    });
    const uri = "/src/services/core.ts";
    const { createDailyCore } = await import(/* @vite-ignore */ uri);
    const core = createDailyCore();
    try {
      let rejected = "",
        notStarted = "";
      try {
        await core.init(fixture.date, 1, { ...rules, hash: "forged" });
      } catch (error) {
        rejected = (error as Error).message;
      }
      try {
        await core.snapshot();
      } catch (error) {
        notStarted = (error as Error).message;
      }
      const recovered = await core.init(fixture.date, 1, rules);
      return { errors, versions, exactView, rejected, notStarted, recovered };
    } finally {
      core.dispose();
    }
  }, acceptedDaily);
  expect(actual.errors).toEqual(Array(11).fill("malformed"));
  expect(actual.versions).toEqual(Array(6).fill("unsupported_version"));
  expect(actual.exactView).toEqual(acceptedDaily.initial);
  expect(actual.rejected).toBe("malformed");
  expect(actual.notStarted).toBe("not_started");
  expect(actual.recovered).toEqual(acceptedDaily.initial);
});
test("daily native/WASM completed replay and production worker share the same public solo contract", async ({
  page,
}) => {
  await page.goto("/en/settings");
  const result = await page.evaluate(async (fixture) => {
    const wasmUri = "/core/liar_wasm.js";
    const wasm = await import(/* @vite-ignore */ wasmUri);
    await wasm.default();
    const session = new wasm.DailySession(fixture.date, fixture.seed_version);
    const initial = JSON.parse(session.snapshot());
    const steps = fixture.events.map((event) =>
      JSON.parse(session.step(event.input, event.time_ms)),
    );
    const replay = JSON.parse(session.replay());
    const before = session.snapshot();
    const clockErrors = [-1, 1.5, NaN, Infinity, 2 ** 32].map((time) => {
      try {
        session.advance(time);
        return "accepted";
      } catch (error) {
        return (error as Error).message;
      }
    });
    const unchanged = before === session.snapshot();
    session.free();
    const versionErrors = [1.5, 2, 65537, NaN].map((version) => {
      try {
        const value = new wasm.DailySession(fixture.date, version);
        value.free();
        return "accepted";
      } catch (error) {
        return (error as Error).message;
      }
    });
    const uri = "/src/services/core.ts";
    const { createDailyCore } = await import(/* @vite-ignore */ uri);
    const core = createDailyCore();
    const workerInitial = await core.init(fixture.date, fixture.seed_version);
    const workerStep = await core.step(
      JSON.parse(fixture.events[0].input),
      fixture.events[0].time_ms,
    );
    const workerReplay = await core.replay();
    core.dispose();
    return {
      initial,
      steps,
      replay,
      clockErrors,
      versionErrors,
      unchanged,
      workerInitial,
      workerStep,
      workerReplay,
    };
  }, daily);
  expect(result.initial).toEqual(daily.initial);
  expect(result.steps).toEqual(daily.events.map((event) => event.expected));
  expect(result.replay).toEqual(daily.replay);
  expect(result.clockErrors).toEqual(Array(5).fill("invalid_time"));
  expect(result.versionErrors).toEqual(Array(4).fill("unsupported_version"));
  expect(result.unchanged).toBe(true);
  expect(result.workerInitial).toEqual(daily.initial);
  expect(result.workerStep).toEqual(daily.events[0].expected);
  expect(result.workerReplay.inputs).toEqual([daily.replay.inputs[0]]);
});
test("UTC daily preserves a running board across midnight and locale, completes locally and shares a real PNG", async ({
  page,
  context,
}, info) => {
  // Traverse all 64 real keyboard inputs, then persist, export and reload on software-rendered CI.
  test.setTimeout(60_000);
  const requests: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  await page.goto("/en/daily?code=ABCD1234#invite");
  await expect(page.getByRole("gridcell")).toHaveCount(256);
  await expect(
    page.getByRole("button", { name: "Accuse", exact: true }),
  ).toHaveCount(0);
  await expect(page.getByTestId("bot-progress")).toHaveCount(0);
  await expect(page.getByRole("grid")).toHaveAttribute(
    "aria-readonly",
    "false",
  );
  const initial = await page.getByTestId("own-progress").textContent();
  await page.clock.setFixedTime(new Date("2026-10-08T00:00:01Z"));
  await page.locator("header select").selectOption("ko");
  await expect(
    page.getByRole("heading", { name: "UTC 데일리 퍼즐" }),
  ).toBeVisible();
  await expect(page.locator(".page-title time")).toHaveText("2026-10-07");
  await expect(page.getByTestId("own-progress")).toHaveText(initial!);
  await page.locator("header select").selectOption("en");
  for (const event of daily.events) {
    const action = JSON.parse(event.input).action;
    await page.locator(`[data-cell="${action.cell}"]`).press("Enter");
    await expect(page.locator(`[data-cell="${action.cell}"]`)).toHaveAttribute(
      "data-state",
      "safe",
    );
  }
  await expect(
    page.getByRole("heading", { name: "Puzzle complete" }),
  ).toBeVisible();
  await expect(page.getByText("First local completion saved.")).toBeVisible();
  await expect(page.locator(".daily-records li")).toHaveCount(1);
  const download = page.waitForEvent("download");
  await page.getByRole("button", { name: "Download PNG" }).click();
  const png = await download;
  expect(png.suggestedFilename()).toBe("liar-daily-2026-10-07.png");
  const path = await png.path();
  const bytes = readFileSync(path!);
  expect(bytes.subarray(1, 4).toString()).toBe("PNG");
  expect(bytes.readUInt32BE(16)).toBe(800);
  expect(bytes.readUInt32BE(20)).toBe(600);
  if (info.project.name === "chromium") {
    await context.grantPermissions(["clipboard-read", "clipboard-write"]);
    await page.getByRole("button", { name: "Copy result" }).click();
    await expect(page.getByText("Result ready.")).toBeVisible();
    const text = await page.evaluate(() => navigator.clipboard.readText());
    expect(text).toContain("2026-10-07 UTC");
    expect(text).toContain("Unverified");
    expect(text).not.toContain(daily.initial.metadata.seed);
    expect(text).not.toMatch(/nickname|replay|command_id|mine|seed/);
  }
  if (process.env.LIAR_CAPTURE_DESIGN === "1") {
    mkdirSync(".tmp/screenshots", { recursive: true });
    await page.evaluate(() => window.scrollTo(0, 0));
    await page.screenshot({
      path: `.tmp/screenshots/13-daily-${info.project.name}.png`,
      fullPage: true,
    });
  }
  await page.reload();
  await expect(page.locator(".daily-records li")).toHaveCount(1);
  await expect(page.locator(".page-title time")).toHaveText("2026-10-08");
  expect(
    requests.filter((uri) => unrequestedServiceCall(uri, page.url())),
  ).toEqual([]);
  expect(page.url()).toContain("code=ABCD1234#invite");
});
test("denied storage and clipboard keep the solo puzzle operable and UTC timeout never saves completion", async ({
  page,
}) => {
  await page.clock.install({ time: new Date("2026-10-07T23:59:59Z") });
  await page.addInitScript(() => {
    Object.defineProperty(window, "indexedDB", {
      get: () => {
        throw Error("denied");
      },
    });
    Object.defineProperty(navigator, "clipboard", { value: undefined });
  });
  await page.goto("/en/daily");
  await expect(page.getByRole("gridcell")).toHaveCount(256);
  await expect(
    page.getByText(
      "Local storage is limited. Some records may last only in this tab.",
    ),
  ).toBeVisible();
  await page.getByRole("button", { name: "Copy result" }).click();
  await expect(
    page.getByText(
      "Sharing is unavailable or cancelled. Try copying the result or downloading a PNG.",
    ),
  ).toBeVisible();
  await page.clock.fastForward(243000);
  await expect(
    page.getByRole("heading", { name: "Puzzle not complete" }),
  ).toBeVisible();
  await expect(page.locator(".daily-records li")).toHaveCount(0);
  await expect(page.getByRole("grid")).toHaveAttribute("aria-readonly", "true");
  await page.getByRole("button", { name: "New puzzle" }).click();
  await expect(page.getByRole("gridcell")).toHaveCount(256);
});
