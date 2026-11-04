import { expect, test, type BrowserContext } from "@playwright/test";
import { mkdir, readFile } from "node:fs/promises";
export function registerConsentControlTests(
  approve: (context: BrowserContext) => Promise<void>,
) {
  test("privacy controls use real session withdrawal in eight locales without optional collection", async ({
    browser,
  }) => {
    const context = await browser.newContext({
      baseURL: "https://localhost:8443",
      ignoreHTTPSErrors: true,
      viewport: { width: 360, height: 844 },
    });
    try {
      await approve(context);
      const page = await context.newPage();
      const optional: string[] = [];
      page.on("request", (request) => {
        if (request.url().includes("/api/v1/events/optional"))
          optional.push(request.url());
      });
      await page.goto("/en/login?return_path=me");
      await page.getByRole("button", { name: "Continue with Google" }).click();
      await expect(page).toHaveURL(/\/en\/onboarding\?return_path=me/);
      await mkdir(".tmp/my-info-layout", { recursive: true });
      for (const width of [360, 960]) {
        await page.setViewportSize({ width, height: 844 });
        for (const locale of [
          "en",
          "ko",
          "ja",
          "zh-CN",
          "es",
          "pt-BR",
          "de",
          "fr",
        ]) {
          await page.request.post("/__fixture/advance", {
            data: { seconds: 1 },
          });
          await page.goto(`/${locale}/me/privacy`);
          await expect(page.getByTestId("me-server-consent")).toBeVisible();
          const button = page.getByTestId("consent-withdraw-all");
          await expect(button).toHaveAccessibleName(/\S/);
          await expect(button).toBeEnabled();
          const response = page.waitForResponse(
            (r) =>
              r.url().endsWith("/api/v1/me/consent/withdraw") &&
              r.request().method() === "POST",
          );
          await button.focus();
          await button.press("Enter");
          const withdrawn = await response;
          expect(withdrawn.status()).toBe(200);
          expect(withdrawn.request().postDataJSON()).toEqual({
            v: 1,
            purpose: "all",
          });
          expect(withdrawn.headers()["cache-control"]).toBe("no-store");
          const messages = JSON.parse(
            await readFile(
              `apps/web/src/locales/${locale}/common.json`,
              "utf8",
            ),
          );
          const view = page.getByTestId("me-server-consent");
          await expect(view).toBeVisible();
          await expect(
            view.getByText(messages["me.status.current"], { exact: true }),
          ).toBeVisible();
          await expect(
            view.getByText(messages["me.choice.denied"], { exact: true }),
          ).toHaveCount(2);
          await expect(button).toBeEnabled();
          expect(
            await page.evaluate(
              () => document.documentElement.scrollWidth <= innerWidth,
            ),
          ).toBe(true);
          await page.screenshot({
            path: `.tmp/my-info-layout/consent190-${locale}-${width}.png`,
            fullPage: true,
          });
        }
      }
      expect(optional).toEqual([]);
      await page.goto("/en/rules");
      await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
      expect(optional).toEqual([]);
    } finally {
      await context.close();
    }
  });
}
