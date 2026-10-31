import { expect, test } from "@playwright/test";
import { mkdirSync } from "node:fs";
import { unrequestedServiceCall } from "./network";
test("the next tutorial action and its target are clear before long instructions", async ({
  page,
}, info) => {
  await page.setViewportSize({
    width: info.project.name === "mobile" ? 360 : 1280,
    height: 800,
  });
  await page.goto("/ko/tutorial?return=%2Fdaily");
  const action = page.getByRole("button", { name: /C3.*열기/ });
  await expect(action).toBeInViewport({ ratio: 1 });
  const target = page.getByRole("gridcell", { name: "C3, 닫힌 칸" });
  await expect(target).toHaveAttribute("aria-current", "step");
  await expect(target).toHaveAttribute("data-hint", "C3");
  await expect(target).toHaveAttribute("aria-selected", "true");
  await expect(page.getByText(/위에서 3번째, 왼쪽에서 3번째/)).toBeVisible();
  const basics = page.locator("details.play-basics");
  const actionBounds = await action.boundingBox();
  const basicsBounds = await basics.boundingBox();
  expect(actionBounds!.y + actionBounds!.height).toBeLessThan(basicsBounds!.y);
  expect(
    await target.evaluate((cell) => getComputedStyle(cell, "::after").content),
  ).toBe('"C3"');
  if (process.env.LIAR_CAPTURE_DESIGN) {
    mkdirSync(".tmp/tutorial-guidance", { recursive: true });
    await page.screenshot({
      path: `.tmp/tutorial-guidance/first-${info.project.name}.png`,
      fullPage: true,
    });
  }
  if (info.project.name === "mobile") await target.tap();
  else await target.click();
  await expect(
    page.getByRole("heading", { name: "2. 공격 보내기" }),
  ).toBeVisible();
  await expect(page.locator('[aria-current="step"]')).toHaveCount(0);
  await page.getByRole("button", { name: "공격하기", exact: true }).click();
  await expect(
    page.getByRole("gridcell", { name: "C1, 닫힌 칸" }),
  ).toHaveAttribute("data-hint", "C1");
  await page.getByRole("button", { name: /C1.*열기/ }).click();
  await expect(
    page.getByRole("gridcell", { name: "C1, 숫자 2" }),
  ).toHaveAttribute("data-hint", "C1");
  await page.getByRole("button", { name: /C1.*지목/ }).click();
  await expect(page.locator('[aria-current="step"]')).toHaveCount(0);
  await expect(
    page.getByRole("gridcell", { name: "C1, 숫자 1" }),
  ).toBeVisible();
  await page.getByRole("button", { name: /다시/ }).click();
  await expect(
    page.getByRole("gridcell", { name: "C3, 닫힌 칸" }),
  ).toHaveAttribute("data-hint", "C3");
  await page.getByRole("button", { name: /건너뛰기/ }).click();
  await expect(page).toHaveURL("/ko/daily");
});
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
  const basics = page
    .locator("details")
    .filter({ hasText: "New to Minesweeper?" });
  const basicsToggle = basics.locator("summary");
  await expect(basics).toHaveJSProperty("open", true);
  await basicsToggle.focus();
  await page.keyboard.press("Enter");
  await expect(basics).toHaveJSProperty("open", false);
  await page.keyboard.press("Enter");
  await expect(basics).toHaveJSProperty("open", true);
  if (info.project.name === "mobile") {
    await basicsToggle.tap();
    await expect(basics).toHaveJSProperty("open", false);
    await basicsToggle.tap();
    await expect(basics).toHaveJSProperty("open", true);
  }
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
  await expect(basics).toHaveJSProperty("open", false);
  await basicsToggle.focus();
  await page.keyboard.press("Enter");
  await expect(basics).toHaveJSProperty("open", true);
  await page.locator("header select").selectOption("ko");
  await expect(
    page.getByRole("heading", { name: "2. 공격 보내기" }),
  ).toBeVisible();
  await expect(page.getByTestId("gauge")).toHaveText("1 / 1");
  const translatedBasics = page
    .locator("details")
    .filter({ hasText: "지뢰찾기가 처음인가요?" });
  await expect(translatedBasics).toHaveJSProperty("open", true);
  await page.locator("header select").selectOption("en");
  await basicsToggle.focus();
  await page.keyboard.press("Enter");
  await expect(basics).toHaveJSProperty("open", false);
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
  await expect
    .poll(() => page.evaluate(() => localStorage.getItem("liar.tutorial.v1")))
    .toBe('"complete"');
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
  await expect(basics).toHaveJSProperty("open", true);
});

test("beginner guidance is readable in eight languages at 360px and shared with the rules page", async ({
  page,
}, info) => {
  const titles = {
    en: "New to Minesweeper?",
    ko: "지뢰찾기가 처음인가요?",
    ja: "マインスイーパーは初めてですか？",
    "zh-CN": "第一次玩扫雷？",
    es: "¿Es tu primera vez en Buscaminas?",
    "pt-BR": "Primeira vez no Campo Minado?",
    de: "Neu bei Minesweeper?",
    fr: "Première partie de Démineur ?",
  };
  const requests: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  await page.setViewportSize({ width: 360, height: 800 });
  await page.goto("/en/tutorial?code=ABCD1234#invite");
  for (const [locale, title] of Object.entries(titles)) {
    await page.locator("header select").selectOption(locale);
    const basics = page.locator("details").filter({ hasText: title });
    await expect(basics.locator("summary")).toBeVisible();
    await expect(basics).toHaveJSProperty("open", true);
    await expect(basics.getByRole("term")).toHaveCount(4);
    await expect(page.getByRole("gridcell")).toHaveCount(9);
    await expect(page.getByTestId("gauge")).toHaveText("0 / 1");
    await expect(
      page.locator('.board-cell[aria-current="step"]'),
    ).toHaveAttribute("data-hint", "C3");
    await expect(page.locator(".tutorial-cell-hint")).toContainText("3");
    await expect(page.locator(".lesson-card .button--primary")).toBeInViewport({
      ratio: 1,
    });
    for (const button of await page.locator(".board-modes > button").all()) {
      expect(
        await button.evaluate((element) => {
          const range = document.createRange();
          range.selectNodeContents(element);
          const text = Array.from(range.getClientRects());
          const button = element.getBoundingClientRect();
          return (
            text.length === 1 &&
            text[0].left >= button.left &&
            text[0].right <= button.right &&
            button.width >= 44 &&
            button.height >= 44
          );
        }),
      ).toBe(true);
    }
    await expect(page).toHaveURL(
      new RegExp(`/${locale}/tutorial\\?code=ABCD1234#invite$`),
    );
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    expect(
      await page.evaluate(() => localStorage.getItem("liar.tutorial.v1")),
    ).toBe(null);
    if (process.env.LIAR_CAPTURE_DESIGN && ["en", "ko"].includes(locale)) {
      mkdirSync(".tmp/tutorial163", { recursive: true });
      await page.screenshot({
        path: `.tmp/tutorial163/basics-${info.project.name}-${locale}.png`,
        fullPage: true,
      });
      await page.locator(".board-bottom").screenshot({
        path: `.tmp/tutorial163/controls-${info.project.name}-${locale}.png`,
      });
    }
  }
  await page.goto("/en/rules");
  const rulesBasics = page.locator("details").filter({ hasText: titles.en });
  await expect(rulesBasics).toHaveJSProperty("open", true);
  await expect(
    rulesBasics.getByText(
      /up to eight neighboring squares, including diagonals/,
    ),
  ).toBeVisible();
  await expect(
    page.getByRole("link", { name: "Replay tutorial" }),
  ).toBeVisible();
  expect(requests.some((uri) => unrequestedServiceCall(uri, page.url()))).toBe(
    false,
  );
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
  // The countdown continues while the DOM grid assertion is being delivered.
  await expect(page.getByText(/Starting in [123] s/)).toBeVisible();
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
