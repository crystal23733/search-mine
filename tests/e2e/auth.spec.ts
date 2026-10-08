import { test, expect, type BrowserContext, type Page } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { mkdir } from "node:fs/promises";
const origin = "https://localhost:8443";
const providers = ["Google", "Apple", "Kakao", "Naver"] as const;
test.describe.configure({ mode: "serial" }); // The fixture's explicit clock control must not overlap accounts.
async function approve(context: BrowserContext, cancel = false) {
  const subject = randomUUID();
  await context.route(
    /^https:\/\/(accounts\.google\.com|appleid\.apple\.com|kauth\.kakao\.com|nid\.naver\.com)\//,
    async (route) => {
      const url = new URL(route.request().url());
      const provider =
        url.hostname === "accounts.google.com"
          ? "google"
          : url.hostname === "appleid.apple.com"
            ? "apple"
            : url.hostname === "kauth.kakao.com"
              ? "kakao"
              : "naver";
      const callback = new URL(`${origin}/api/v1/auth/${provider}/callback`);
      callback.searchParams.set("state", url.searchParams.get("state")!);
      callback.searchParams.set(
        cancel ? "error" : "code",
        cancel ? "access_denied" : `fixture-${subject}`,
      );
      if (provider === "google")
        callback.searchParams.set("iss", "https://accounts.google.com");
      await route.fulfill({
        status: 302,
        headers: { location: callback.href },
      });
    },
  );
}
async function login(page: Page, provider = "Google") {
  await page.goto("/en/login");
  await page.getByRole("button", { name: `Continue with ${provider}` }).click();
  await expect(page).toHaveURL(/\/en\/onboarding\?/);
  await page.getByRole("textbox", { name: "Nickname" }).fill("e\u0301探偵");
  await page.getByRole("button", { name: "Save and skip tutorial" }).click();
  await expect(page).toHaveURL(`${origin}/en/`);
  await page.goto("/en/settings");
  await expect(
    page.getByRole("button", { name: "Download my data" }),
  ).toBeVisible();
}
test("four providers work through real SQL, HTTPS cookies and authoritative Unicode nickname on desktop/mobile", async ({
  browser,
}) => {
  for (const viewport of [
    { width: 1440, height: 900 },
    { width: 390, height: 844 },
  ]) {
    for (const provider of providers) {
      const context = await browser.newContext({
        baseURL: origin,
        viewport,
        ignoreHTTPSErrors: true,
      });
      try {
        await approve(context);
        const page = await context.newPage();
        await login(page, provider);
        await expect(page.getByText("é探偵", { exact: true })).toBeVisible();
        const cookies = await context.cookies();
        const session = cookies.find((c) => c.name === "__Host-liar_session")!;
        expect(session).toMatchObject({
          httpOnly: true,
          secure: true,
          sameSite: "Lax",
          path: "/",
        });
        expect(await page.evaluate(() => document.cookie)).not.toContain(
          "__Host-liar_",
        );
        const bootstrap = await page.request.get("/api/v1/auth/bootstrap");
        expect(bootstrap.headers()["cache-control"]).toBe("no-store");
        const account = (await bootstrap.json()).account;
        expect(Object.keys(account).sort()).toEqual(["id", "nickname"]);
        expect(
          await page.evaluate(() =>
            JSON.stringify({ ...localStorage, ...sessionStorage }),
          ),
        ).not.toMatch(/csrf|fixture-|__Host-liar|password|email/);
        expect(
          await page.evaluate(
            () => document.documentElement.scrollWidth <= innerWidth,
          ),
        ).toBe(true);
        if (provider === "Google") {
          await mkdir(".tmp/auth-layout", { recursive: true });
          await page.screenshot({
            path: `.tmp/auth-layout/settings-${viewport.width}.png`,
            fullPage: true,
          });
        }
      } finally {
        await context.close();
      }
    }
  }
});
test("export, linking, last-provider guard, unlink session revocation and logout use production routes", async ({
  page,
  context,
}) => {
  await approve(context);
  await login(page);
  const download = page.waitForEvent("download");
  await page.getByRole("button", { name: "Download my data" }).click();
  const artifact = await download;
  const stream = await artifact.createReadStream();
  const chunks: Buffer[] = [];
  for await (const chunk of stream!) chunks.push(Buffer.from(chunk));
  const exported = JSON.parse(Buffer.concat(chunks).toString("utf8"));
  expect(Object.keys(exported).sort()).toEqual([
    "account",
    "created_at",
    "identities",
    "last_seen_at",
  ]);
  expect(exported.account.nickname).toBe("é探偵");
  expect(JSON.stringify(exported)).not.toMatch(
    /subject|digest|credential|csrf|password|email/,
  );
  await expect(
    page.getByRole("button", { name: "Disconnect Google" }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "Connect Apple" }).click();
  await expect(page).toHaveURL(`${origin}/en/settings`);
  await expect(
    page.getByRole("button", { name: "Disconnect Apple" }),
  ).toBeEnabled();
  await page.getByRole("button", { name: "Disconnect Apple" }).click();
  await page.getByRole("button", { name: "Disconnect and sign out" }).click();
  await expect(page.locator("a.account-card")).toBeVisible();
  expect(
    (await context.cookies()).some((c) => c.name === "__Host-liar_session"),
  ).toBe(false);
});
test("expired authentication requires a fresh provider proof and a new explicit deletion confirmation", async ({
  page,
  context,
}) => {
  await approve(context);
  await login(page);
  await page.request.post("/__fixture/advance", { data: { seconds: 301 } });
  await page.getByRole("button", { name: "Delete account" }).click();
  await page
    .getByRole("button", { name: "Delete account permanently" })
    .click();
  await expect(
    page.getByRole("heading", { name: "Confirm your sign-in again" }),
  ).toBeVisible();
  await expect(page.getByRole("dialog")).not.toBeVisible();
  await page.getByRole("button", { name: "Confirm with Google" }).click();
  await expect(page).toHaveURL(`${origin}/en/settings`);
  await expect(page.getByText("é探偵", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Delete account" }).click();
  const dialog = page.getByRole("dialog", { name: "Delete account" });
  await expect(dialog.getByRole("button", { name: "Close" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(dialog).not.toBeVisible();
  await page.getByRole("button", { name: "Delete account" }).click();
  await page
    .getByRole("button", { name: "Delete account permanently" })
    .click();
  await expect(page.locator("a.account-card")).toBeVisible();
  expect(
    (await page.request.get("/api/v1/auth/bootstrap").then((r) => r.json()))
      .account,
  ).toBeNull();
});
test("provider cancellation and authentication outage leave guest practice usable in every locale", async ({
  page,
  context,
}) => {
  await approve(context, true);
  await page.goto("/en/login");
  await page.getByRole("button", { name: "Continue with Google" }).click();
  await expect(page).toHaveURL(/\/en\/login\?error=auth_failed/);
  await expect(page.getByRole("alert")).toBeVisible();
  await page.route("**/api/v1/auth/bootstrap", (route) =>
    route.fulfill({ status: 503 }),
  );
  for (const locale of ["en", "ko", "ja", "zh-CN", "es", "pt-BR", "de", "fr"]) {
    await page.goto(`/${locale}/login`);
    await expect(page.locator("html")).toHaveAttribute("lang", locale);
    await expect(page.locator(".auth-local")).toHaveAttribute(
      "href",
      `/${locale}/practice`,
    );
    await expect(page.locator(".provider-button").first()).toBeDisabled();
    await expect(page.getByRole("alert")).toBeVisible();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
  }
});
