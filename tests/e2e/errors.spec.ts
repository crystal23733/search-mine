import { expect, test } from "@playwright/test";
import { mkdir, readFile } from "node:fs/promises";
const locales = ["en", "ko", "ja", "zh-CN", "es", "pt-BR", "de", "fr"];
test.beforeEach(async ({ page }, info) => {
  if (info.project.name === "mobile")
    await page.setViewportSize({ width: 360, height: 844 });
  await page.addInitScript(() =>
    localStorage.setItem("liar.tutorial.v1", '"skipped"'),
  );
});
test("error documents expose real origin statuses and eight locale screens without reflecting private URL input", async ({
  page,
  request,
}, info) => {
  await mkdir(".tmp/error-layout", { recursive: true });
  for (const origin of ["http://127.0.0.1:5173", "http://127.0.0.1:4173"]) {
    for (const locale of locales) {
      const messages = JSON.parse(
        await readFile(`apps/web/src/locales/${locale}/common.json`, "utf8"),
      ) as Record<string, string>;
      for (const status of [403, 404] as const) {
        const path = status === 403 ? "/403" : "/not-a-page",
          url = `${origin}/${locale}${path}?token=private-value#secret-fragment`;
        // RequestContext observes the origin independently from SPA and cached service-worker navigation.
        const response = await request.get(url, {
          headers: { Accept: "text/html" },
        });
        expect(response.status()).toBe(status);
        expect(response.headers()["cache-control"]).toBe("no-store");
        expect(response.headers()["x-robots-tag"]).toBe("noindex, noarchive");
        expect(response.headers()["x-content-type-options"]).toBe("nosniff");
        expect(await response.text()).not.toContain("private-value");
        await page.goto(url);
        await expect(
          page.getByRole("heading", {
            name: messages[
              status === 403 ? "error.forbiddenTitle" : "error.notFoundTitle"
            ],
            level: 1,
          }),
        ).toBeVisible();
        await expect(page.locator("main")).toContainText(String(status));
        await expect(page.locator("body")).not.toContainText("private-value");
        await expect(page.locator('meta[name="robots"]')).toHaveAttribute(
          "content",
          "noindex, noarchive",
        );
        await expect(page).toHaveTitle(new RegExp(String(status)));
        const home = page.getByRole("link", {
          name: messages["error.home"],
          exact: true,
        });
        await expect(home).toHaveAttribute("href", `/${locale}/`);
        if (status === 403)
          await expect(
            page.getByRole("link", {
              name: messages["error.account"],
              exact: true,
            }),
          ).toHaveAttribute("href", `/${locale}/me/account`);
        expect(
          await page.evaluate(
            () => document.documentElement.scrollWidth <= innerWidth,
          ),
        ).toBe(true);
        if (locale === "ko" && origin.endsWith("4173"))
          await page.screenshot({
            path: `.tmp/error-layout/${status}-${info.project.name}.png`,
            fullPage: true,
          });
      }
    }
  }
});
test("HEAD error bodies are empty while normal documents and assets keep their original responses", async ({
  page,
  request,
}) => {
  const origin = "http://127.0.0.1:4173";
  const head = await request.fetch(`${origin}/ko/403`, {
    method: "HEAD",
    headers: { Accept: "text/html" },
  });
  expect(head.status()).toBe(403);
  expect(await head.body()).toHaveLength(0);
  const normal = await request.get(`${origin}/en/rules`, {
    headers: { Accept: "text/html" },
  });
  expect(normal.status()).toBe(200);
  expect(normal.headers()["x-robots-tag"]).toBeUndefined();
  await page.goto(`${origin}/ko/404?token=private-value#secret-fragment`);
  const home = page.getByRole("link", { name: "홈으로 돌아가기", exact: true });
  await home.focus();
  await home.press("Enter");
  await expect(page).toHaveURL(`${origin}/ko/`);
  await expect(page.locator('meta[name="robots"]')).toHaveCount(0);
  const source = await page
    .locator('script[type="module"]')
    .first()
    .getAttribute("src");
  if (!source) throw Error("Missing built entry");
  const asset = await request.get(new URL(source, origin).href, {
    headers: { Accept: "text/html" },
  });
  expect(asset.status()).toBe(200);
  expect(asset.headers()["content-type"]).not.toContain("text/html");
  const nonHtml = await request.get(`${origin}/en/missing`, {
    headers: { Accept: "application/json" },
  });
  expect(await nonHtml.text()).not.toContain('id="app"');
});
