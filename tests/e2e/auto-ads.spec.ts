import { expect, test, type Page } from "@playwright/test";
import type { install } from "../../apps/web/test/auto-ads-harness";
type FixtureWindow = Window & {
  __autoAdsFixture?: ReturnType<typeof install>;
  __testAdExecutions?: number;
};
const sdk = "https://pagead2.googlesyndication.com/**";
const source =
  "window.__testAdExecutions = (window.__testAdExecutions || 0) + 1;";
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() =>
    localStorage.setItem("liar.tutorial.v1", '"skipped"'),
  );
});
async function fixture(page: Page, allowed = true) {
  const uri = new URL("../../apps/web/test/auto-ads.html", import.meta.url);
  await page.goto(`/@fs${uri.pathname}`);
  await expect(
    page.getByRole("heading", { name: "Public document fixture" }),
  ).toBeVisible();
  await page.evaluate(async (allowed) => {
    history.replaceState(null, "", "/ko/rules");
    const uri = "/test/auto-ads-harness.ts";
    const module = await import(/* @vite-ignore */ uri);
    (window as FixtureWindow).__autoAdsFixture = module.install(allowed);
  }, allowed);
}
async function game(page: Page) {
  await page.evaluate(() =>
    (window as FixtureWindow).__autoAdsFixture!.go("/practice"),
  );
  await expect(page).toHaveURL(/\/ko\/practice$/);
  await expect(page.locator("#app")).toBeVisible();
  expect(
    await page.evaluate(() => (window as FixtureWindow).__testAdExecutions),
  ).toBeUndefined();
  await expect(page.locator("script[data-liar-auto-ads]")).toHaveCount(0);
}

test("unconfigured application stays free of ad scripts across eight locales", async ({
  page,
}) => {
  const requests: string[] = [];
  page.on("request", (request) => {
    if (request.url().includes("googlesyndication.com"))
      requests.push(request.url());
  });
  for (const locale of ["en", "ko", "ja", "zh-CN", "es", "pt-BR", "de", "fr"]) {
    await page.goto(`/${locale}/rules`);
    await expect(page.locator("#app")).toBeVisible();
    await expect(page.locator("script[data-liar-auto-ads]")).toHaveCount(0);
  }
  expect(requests).toEqual([]);
});

test("one automatic script stays on safe navigation and cannot follow a game into its fresh document or back", async ({
  page,
}) => {
  let requests = 0;
  await page.route(sdk, async (route) => {
    requests++;
    await route.fulfill({ contentType: "text/javascript", body: source });
  });
  await fixture(page);
  await expect
    .poll(() =>
      page.evaluate(() => (window as FixtureWindow).__testAdExecutions),
    )
    .toBe(1);
  await page.evaluate(() => {
    (window as FixtureWindow).__autoAdsFixture!.go("/");
    (window as FixtureWindow).__autoAdsFixture!.go("/rules");
  });
  expect(requests).toBe(1);
  await game(page);
  await page.goBack();
  await expect(page).toHaveURL(/\/ko\/rules$/);
  await expect(page.locator("#app")).toBeVisible();
  expect(
    await page.evaluate(() => (window as FixtureWindow).__testAdExecutions),
  ).toBeUndefined();
  expect(requests).toBe(1);
});

test("denied purpose starts no script, then withdrawal replaces the SDK document", async ({
  page,
}) => {
  let requests = 0;
  await page.route(sdk, async (route) => {
    requests++;
    await route.fulfill({ contentType: "text/javascript", body: source });
  });
  await fixture(page, false);
  expect(requests).toBe(0);
  await expect(page.locator("script[data-liar-auto-ads]")).toHaveCount(0);
  await page.evaluate(() =>
    (window as FixtureWindow).__autoAdsFixture!.consent(true),
  );
  await expect
    .poll(() =>
      page.evaluate(() => (window as FixtureWindow).__testAdExecutions),
    )
    .toBe(1);
  await Promise.all([
    page.waitForEvent("load"),
    page.evaluate(() =>
      (window as FixtureWindow).__autoAdsFixture!.consent(false),
    ),
  ]);
  await expect(page.locator("#app")).toBeVisible();
  expect(
    await page.evaluate(() => (window as FixtureWindow).__testAdExecutions),
  ).toBeUndefined();
  await expect(page.locator("script[data-liar-auto-ads]")).toHaveCount(0);
  expect(requests).toBe(1);
});

test("blocked and pending scripts do not delay entry into an ad-free game", async ({
  page,
}) => {
  await page.route(sdk, (route) => route.abort("blockedbyclient"));
  await fixture(page);
  await expect(page.locator("script[data-liar-auto-ads]")).toHaveCount(0);
  await game(page);
  await page.unroute(sdk);
  let release: () => void = () => {};
  const hold = new Promise<void>((resolve) => {
    release = resolve;
  });
  const requested = page.waitForRequest((request) =>
    request.url().includes("googlesyndication.com"),
  );
  await page.route(sdk, async (route) => {
    await hold;
    await route
      .fulfill({ contentType: "text/javascript", body: source })
      .catch(() => {});
  });
  await fixture(page);
  await requested;
  try {
    await game(page);
  } finally {
    release();
  }
});

test("fixed skip link keeps the automatic script on the same public document", async ({
  page,
}) => {
  await page.route(sdk, (route) =>
    route.fulfill({ contentType: "text/javascript", body: source }),
  );
  await fixture(page);
  await expect
    .poll(() =>
      page.evaluate(() => (window as FixtureWindow).__testAdExecutions),
    )
    .toBe(1);
  await page.getByRole("link", { name: "Skip to content" }).click();
  await expect(page).toHaveURL(/\/ko\/rules#main$/);
  await expect(page.locator("#main")).toBeFocused();
  expect(
    await page.evaluate(() => (window as FixtureWindow).__testAdExecutions),
  ).toBe(1);
});
