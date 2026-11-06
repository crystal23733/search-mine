import { expect, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
const locales = ["en", "ko", "ja", "zh-CN", "es", "pt-BR", "de", "fr"];
test("public locale documents provide translated content without JavaScript", async ({
  browser,
  request,
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
    for (const origin of ["http://127.0.0.1:5173", "http://127.0.0.1:4173"]) {
      for (const locale of locales) {
        const messages = JSON.parse(
          await readFile(`apps/web/src/locales/${locale}/common.json`, "utf8"),
        );
        for (const [path, key] of [
          ["/", "home.title1"],
          ["/rules", "rules.title"],
          ["/storage", "storage.pageTitle"],
        ]) {
          const response = await page.goto(`${origin}/${locale}${path}`);
          expect(response?.status()).toBe(200);
          await expect(page.locator("html")).toHaveAttribute("lang", locale);
          await expect(page.getByRole("heading", { level: 1 })).toContainText(
            messages[key],
          );
          await expect(
            page.getByRole("link", { name: "English", exact: true }),
          ).toHaveAttribute("href", `/en${path}`);
          expect(await page.locator('link[rel="canonical"]').count()).toBe(0);
          expect(
            await page.evaluate(
              () => document.documentElement.scrollWidth <= innerWidth,
            ),
          ).toBe(true);
        }
      }
      for (const path of [
        "/en/me",
        "/ko/login",
        "/fr/friends",
        "/en/rules?token=private-value",
      ]) {
        const response = await request.get(`${origin}${path}`, {
          headers: { Accept: "text/html" },
        });
        const head = await request.fetch(`${origin}${path}`, {
          method: "HEAD",
          headers: { Accept: "text/html" },
        });
        expect(head.status()).toBe(200);
        expect(await head.body()).toHaveLength(0);
        expect(head.headers()["x-robots-tag"]).toBe("noindex, noarchive");
        expect(response.headers()["x-robots-tag"]).toBe("noindex, noarchive");
        expect(response.headers()["cache-control"]).toBe("no-store");
        const html = await response.text();
        expect(html).not.toContain("private-value");
        expect(html).not.toContain('rel="canonical"');
      }
      const robots = await request.get(`${origin}/robots.txt`);
      expect(await robots.text()).toBe("User-agent: *\nDisallow: /\n");
      expect((await request.get(`${origin}/sitemap.xml`)).status()).toBe(404);
    }
    await page.goto("http://127.0.0.1:4173/de/rules");
    await page.keyboard.press("Tab");
    await expect(
      page.getByRole("link", {
        name: JSON.parse(
          await readFile("apps/web/src/locales/de/common.json", "utf8"),
        )["skip"],
        exact: true,
      }),
    ).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.locator("main")).toBeFocused();
    await page.screenshot({
      path: `.tmp/seo202-nojs-${info.project.name}.png`,
      fullPage: true,
    });
  } finally {
    await context.close();
  }
});

test("SPA metadata follows locale, safe skip fragment and private fragments without duplicating static content", async ({
  page,
}) => {
  await page.addInitScript(() =>
    localStorage.setItem("liar.tutorial.v1", '"skipped"'),
  );
  await page.goto("/en/rules");
  await expect(
    page.getByRole("combobox", { name: "Language", exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("heading", { level: 1 })).toHaveCount(1);
  await expect(page).toHaveTitle("How to play | Liar Sweeper");
  await page
    .getByRole("combobox", { name: "Language", exact: true })
    .selectOption("ko");
  await expect(page).toHaveTitle(
    `${JSON.parse(await readFile("apps/web/src/locales/ko/common.json", "utf8"))["rules.title"]} | Liar Sweeper`,
  );
  await expect(page.locator("html")).toHaveAttribute("lang", "ko");
  await page.evaluate(() => {
    location.hash = "private-value";
  });
  await expect(page.locator('meta[name="robots"]')).toHaveAttribute(
    "content",
    "noindex, noarchive",
  );
  expect(await page.locator("head").innerHTML()).not.toContain("private-value");
  await page.evaluate(() => {
    location.hash = "main";
  });
  await expect(page.locator('meta[name="robots"]')).toHaveCount(0);
  await page.getByRole("link", { name: "Liar Sweeper", exact: true }).click();
  await expect(page.getByRole("heading", { level: 1 })).toHaveCount(1);
});
