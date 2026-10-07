import { expect, test } from "@playwright/test";
import { mkdirSync } from "node:fs";
const locales = ["en", "ko", "ja", "zh-CN", "es", "pt-BR", "de", "fr"];
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() =>
    localStorage.setItem("liar.tutorial.v1", '"skipped"'),
  );
});
test("all locale URLs render without overflow at mobile and desktop sizes", async ({
  page,
}, info) => {
  for (const locale of locales) {
    await page.goto(`/${locale}/`);
    await expect(page.locator("html")).toHaveAttribute("lang", locale);
    await expect(
      page.getByRole("link", { name: "Liar Sweeper" }),
    ).toBeVisible();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
  }
  if (info.project.name === "chromium") {
    for (const width of [360, 390, 768, 1024, 1440]) {
      await page.setViewportSize({ width, height: 900 });
      await page.goto("/de/");
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
    }
  }
  if (process.env.LIAR_CAPTURE_DESIGN) {
    mkdirSync(".tmp/screenshots", { recursive: true });
    await page.goto("/ko/");
    await page.screenshot({
      path: `.tmp/screenshots/10-home-${info.project.name}.png`,
      fullPage: true,
    });
  }
});
test("locale switching preserves route/invite state and preference reload", async ({
  page,
}) => {
  await page.goto("/en/settings?code=ABCD1234#preferences");
  await page.locator("header select").selectOption("ko");
  await expect(page).toHaveURL(/\/ko\/settings\?code=ABCD1234#preferences$/);
  await expect(page.locator("html")).toHaveAttribute("lang", "ko");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("lang", "ko");
  await page.locator("header select").selectOption("en");
  await page.getByRole("checkbox", { name: "High contrast" }).check();
  await page.getByRole("checkbox", { name: "Reduce motion" }).check();
  await page.reload();
  await expect(
    page.getByRole("checkbox", { name: "High contrast" }),
  ).toBeChecked();
  await expect(
    page.getByRole("checkbox", { name: "Reduce motion" }),
  ).toBeChecked();
  await expect(page.locator("html")).toHaveAttribute("data-contrast", "high");
  await expect(page.locator("html")).toHaveAttribute("data-motion", "reduce");
});
test("native modal traps keyboard focus, closes with Escape and restores the opener", async ({
  page,
}) => {
  await page.goto("/en/settings");
  const opener = page.getByRole("button", { name: "Essential storage" });
  await opener.click();
  const dialog = page.getByRole("dialog", { name: "Essential storage" });
  await expect(dialog).toBeVisible();
  for (let index = 0; index < 4; index++) {
    await page.keyboard.press("Tab");
    expect(
      await page.evaluate(() =>
        Boolean(document.activeElement?.closest("dialog")),
      ),
    ).toBe(true);
  }
  await page.keyboard.press("Escape");
  await expect(dialog).not.toBeVisible();
  await expect(opener).toBeFocused();
});
test("system reduced-motion is respected and no ad or credential requests start on home", async ({
  page,
}) => {
  const requests: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/en/settings");
  const duration = await page
    .getByRole("checkbox", { name: "Reduce motion" })
    .evaluate(
      (element) => getComputedStyle(element, "::before").transitionDuration,
    );
  expect(parseFloat(duration)).toBeLessThanOrEqual(0.001);
  expect(
    requests.some((url) => /doubleclick|googlesyndication|\/auth\//i.test(url)),
  ).toBe(false);
});
