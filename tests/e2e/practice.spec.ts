import { expect, test } from "@playwright/test";
import { mkdirSync } from "node:fs";
import { unrequestedServiceCall } from "./network";
test("first visit runs actual WASM attack and accusation, preserves locale and completes only training", async ({
  page,
}, info) => {
  const requests: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  await page.goto("/en/?code=ABCD1234#invite");
  await expect(page).toHaveURL(
    /\/en\/tutorial\?code=ABCD1234&return=%2F#invite$/,
  );
  await expect(page.getByRole("gridcell")).toHaveCount(9);
  await expect(page.getByTestId("gauge")).toHaveText("0 / 1");
  await page.locator('[data-cell="2"]').click();
  await expect(
    page.getByText("Follow the current lesson action, then try again."),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "1. Open a safe square" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Open C3" }).click();
  await expect(
    page.getByRole("heading", { name: "2. Send an attack" }),
  ).toBeVisible();
  await expect(page.getByTestId("gauge")).toHaveText("1 / 1");
  await page.locator("header select").selectOption("ko");
  await expect(
    page.getByRole("heading", { name: "2. 공격 보내기" }),
  ).toBeVisible();
  await expect(page.getByTestId("gauge")).toHaveText("1 / 1");
  await page.locator("header select").selectOption("en");
  await page.getByRole("button", { name: "Attack", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "3. Reveal the changed number" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Open C1" }).click();
  await expect(page.locator('[data-cell="2"]')).toHaveAttribute(
    "data-number",
    "2",
  );
  await page.getByRole("button", { name: "Accuse C1" }).click();
  await expect(
    page.getByRole("heading", { name: "You reflected the attack!" }),
  ).toBeVisible();
  await expect(page.locator('[data-cell="2"]')).toHaveAttribute(
    "data-number",
    "1",
  );
  await expect(page.getByText("Correct accusations: 1 / 1")).toBeVisible();
  expect(
    await page.evaluate(() => localStorage.getItem("liar.tutorial.v1")),
  ).toBe('"complete"');
  expect(requests.some((uri) => unrequestedServiceCall(uri, page.url()))).toBe(
    false,
  );
  if (process.env.LIAR_CAPTURE_DESIGN) {
    mkdirSync(".tmp/screenshots", { recursive: true });
    await page.screenshot({
      path: `.tmp/screenshots/12-tutorial-${info.project.name}.png`,
      fullPage: true,
    });
  }
  await page.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(page).toHaveURL(/\/en\/\?code=ABCD1234#invite$/);
  await page.reload();
  await expect(page.getByRole("heading", { level: 1 })).toContainText(
    "win with logic",
  );
  await page.goto("/en/tutorial");
  await expect(page.getByRole("button", { name: "Open C3" })).toBeVisible();
});
test("skip retains invitation and blocks external tutorial redirects even without storage", async ({
  page,
}) => {
  await page.addInitScript(() => {
    Object.defineProperty(Storage.prototype, "setItem", {
      value() {
        throw Error("denied");
      },
    });
  });
  await page.goto(
    "/en/tutorial?code=ABCD1234&return=https%3A%2F%2Fexample.com#invite",
  );
  await page.getByRole("button", { name: "Skip tutorial" }).click();
  await expect(page).toHaveURL(/\/en\/\?code=ABCD1234#invite$/);
  await page
    .getByRole("link", { name: "Practise locally", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Local bot match" }),
  ).toBeVisible();
});
test("all three local bots use the real board, locale preserves progress, restart replaces the session", async ({
  page,
}, info) => {
  await page.addInitScript(() =>
    localStorage.setItem("liar.tutorial.v1", '"skipped"'),
  );
  await page.goto("/en/practice?difficulty=easy");
  await expect(page.getByRole("gridcell")).toHaveCount(256);
  await expect(page.getByText("Starting in 3 s")).toBeVisible();
  let opening = parseInt(
    (await page.getByTestId("bot-progress").textContent())!,
  );
  await expect
    .poll(
      async () =>
        parseInt((await page.getByTestId("bot-progress").textContent())!),
      { timeout: 15000 },
    )
    .toBeGreaterThan(opening);
  const before = await page.getByTestId("own-progress").textContent();
  await page.locator("header select").selectOption("ko");
  await expect(
    page.getByRole("heading", { name: "로컬 봇 대전" }),
  ).toBeVisible();
  await expect(page.getByTestId("own-progress")).toHaveText(before!);
  await expect(page.getByText(/초 후 시작/)).toHaveCount(0);
  await page.locator("header select").selectOption("en");
  for (const difficulty of ["normal", "hard"]) {
    await page.getByLabel("Bot difficulty").selectOption(difficulty);
    await expect(page).toHaveURL(new RegExp(`difficulty=${difficulty}`));
    await expect(page.getByRole("gridcell")).toHaveCount(256);
    await expect(page.getByText(/Starting in [123] s/)).toBeVisible();
    opening = parseInt((await page.getByTestId("bot-progress").textContent())!);
    await expect
      .poll(
        async () =>
          parseInt((await page.getByTestId("bot-progress").textContent())!),
        { timeout: 15000 },
      )
      .toBeGreaterThan(opening);
  }
  if (process.env.LIAR_CAPTURE_DESIGN) {
    mkdirSync(".tmp/screenshots", { recursive: true });
    await page.screenshot({
      path: `.tmp/screenshots/12-match-${info.project.name}.png`,
      fullPage: true,
    });
  }
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await page.getByRole("button", { name: "New match" }).click();
  await expect(page.getByText(/Starting in [123] s/)).toBeVisible();
  await page.getByRole("link", { name: "Home", exact: true }).click();
  await expect(page.getByRole("grid")).toHaveCount(0);
});
test("training worker rejects malformed clocks without advancing lessons", async ({
  page,
}) => {
  await page.goto("/en/settings");
  const result = await page.evaluate(async () => {
    const uri = "/src/services/core.ts";
    const { createTrainingCore } = await import(/* @vite-ignore */ uri);
    const core = createTrainingCore();
    const before = await core.init();
    const errors = [];
    for (const time of [-1, 1.5, NaN, Infinity, 2 ** 32]) {
      try {
        await core.advance(time);
        errors.push("");
      } catch (error) {
        errors.push(error instanceof Error ? error.message : "");
      }
    }
    const after = await core.snapshot();
    core.dispose();
    return { before, after, errors };
  });
  expect(result.errors).toEqual(Array(5).fill("invalid_time"));
  expect(result.after).toEqual(result.before);
});
