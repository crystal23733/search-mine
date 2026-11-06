import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
const locales = ["en", "ko", "ja", "zh-CN", "es", "pt-BR", "de", "fr"];
test("tutorial guide is an eight-language public document without JavaScript", async ({
  browser,
}, info) => {
  const context = await browser.newContext({
    javaScriptEnabled: false,
    viewport:
      info.project.name === "mobile"
        ? { width: 360, height: 844 }
        : { width: 1440, height: 900 },
  });
  const page = await context.newPage();
  try {
    for (const origin of ["http://127.0.0.1:5173", "http://127.0.0.1:4173"])
      for (const locale of locales) {
        const messages = JSON.parse(
          await readFile(`apps/web/src/locales/${locale}/common.json`, "utf8"),
        );
        const response = await page.goto(`${origin}/${locale}/learn`);
        expect(response?.status()).toBe(200);
        await expect(page.locator("html")).toHaveAttribute("lang", locale);
        await expect(
          page.getByRole("heading", {
            name: messages["learn.title"],
            level: 1,
          }),
        ).toBeVisible();
        await expect(
          page.getByRole("link", {
            name: messages["learn.start"],
            exact: true,
          }),
        ).toHaveAttribute("href", `/${locale}/tutorial`);
        await expect(
          page.getByRole("link", { name: "English", exact: true }),
        ).toHaveAttribute("href", "/en/learn");
        expect(
          await page.evaluate(
            () => document.documentElement.scrollWidth <= innerWidth,
          ),
        ).toBe(true);
        if (locale === "de" && origin.endsWith("4173"))
          await page.screenshot({
            path: `.tmp/learn204-nojs-${info.project.name}.png`,
            fullPage: true,
          });
      }
  } finally {
    await context.close();
  }
});

test("a first visit keeps the guide read-only and enters the real tutorial only through a clean keyboard link", async ({
  page,
}, info) => {
  if (info.project.name === "mobile")
    await page.setViewportSize({ width: 360, height: 844 });
  await page.addInitScript(() => localStorage.removeItem("liar.tutorial.v1"));
  const sdkRequests: string[] = [];
  page.on("request", (request) => {
    if (request.url().includes("googlesyndication.com"))
      sdkRequests.push(request.url());
  });
  for (const locale of locales) {
    const messages = JSON.parse(
      await readFile(`apps/web/src/locales/${locale}/common.json`, "utf8"),
    );
    await page.goto(`/${locale}/learn?token=private-value#private-fragment`);
    await expect(
      page.getByRole("combobox", { name: messages.language, exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole("heading", { name: messages["learn.title"], level: 1 }),
    ).toBeVisible();
    await expect(page).toHaveURL(
      new RegExp(`/${locale}/learn\\?token=private-value#private-fragment$`),
    );
    await expect(page.getByRole("gridcell")).toHaveCount(0);
    expect(
      await page.evaluate(() => localStorage.getItem("liar.tutorial.v1")),
    ).toBeNull();
    await expect(
      page.getByRole("link", { name: messages["learn.start"], exact: true }),
    ).toHaveAttribute("href", `/${locale}/tutorial`);
    expect(await page.locator("head").innerHTML()).not.toContain(
      "private-value",
    );
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
  }
  await page.goto("/en/rules?token=private-value#private-fragment");
  await expect(
    page.getByRole("combobox", { name: "Language", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("link", { name: "Tutorial guide", exact: true })
    .first()
    .click();
  await expect(page).toHaveURL(/\/en\/learn$/);
  await expect(
    page.getByRole("combobox", { name: "Language", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("combobox", { name: "Language", exact: true })
    .selectOption("ko");
  await expect(
    page.getByRole("heading", { name: "튜토리얼 안내", level: 1 }),
  ).toBeVisible();
  const start = page.getByRole("link", { name: "튜토리얼 시작", exact: true });
  await start.focus();
  await start.press("Enter");
  await expect(page).toHaveURL(/\/ko\/tutorial$/);
  await expect(page.getByRole("gridcell")).toHaveCount(9);
  expect(
    await page.evaluate(() => localStorage.getItem("liar.tutorial.v1")),
  ).toBeNull();
  expect(sdkRequests).toEqual([]);
  await page.addInitScript(() => {
    for (const method of ["getItem", "setItem"] as const)
      Object.defineProperty(Storage.prototype, method, {
        value: () => {
          throw new DOMException("Storage denied", "SecurityError");
        },
      });
  });
  await page.goto("/ja/learn");
  const japanese = JSON.parse(
    await readFile("apps/web/src/locales/ja/common.json", "utf8"),
  );
  await expect(
    page.getByRole("combobox", { name: japanese.language, exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: japanese["learn.title"], level: 1 }),
  ).toBeVisible();
  await expect(page.getByRole("gridcell")).toHaveCount(0);
  await expect(
    page.getByRole("link", { name: japanese["learn.start"], exact: true }),
  ).toHaveAttribute("href", "/ja/tutorial");
  expect(sdkRequests).toEqual([]);
});
