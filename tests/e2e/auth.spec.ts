import { test, expect, type BrowserContext, type Page } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { mkdir, readFile } from "node:fs/promises";
import { registerOnlineQuickTests } from "./helpers/online-quick";
const origin = "https://localhost:8443";
const providers = ["Google", "Apple", "Kakao", "Naver"] as const;
test.describe.configure({ mode: "serial" }); // The fixture's explicit clock control must not overlap accounts.
registerOnlineQuickTests(approve, login);

const invitationLocales = [
  "en",
  "ko",
  "ja",
  "zh-CN",
  "es",
  "pt-BR",
  "de",
  "fr",
];
for (const [index, locale] of invitationLocales.entries()) {
  test(`invitation keeps ${locale} through ${providers[index % 4]} and nickname/tutorial with storage denied`, async ({
    browser,
  }) => {
    const context = await browser.newContext({
      baseURL: origin,
      viewport:
        index % 2 ? { width: 390, height: 844 } : { width: 1440, height: 900 },
      ignoreHTTPSErrors: true,
    });
    try {
      await context.addInitScript(() => {
        for (const method of ["getItem", "setItem"] as const)
          Object.defineProperty(Storage.prototype, method, {
            value: () => {
              throw new DOMException("Storage denied", "SecurityError");
            },
          });
      });
      await approve(context);
      const messages = JSON.parse(
        await readFile(
          new URL(
            `../../apps/web/src/locales/${locale}/common.json`,
            import.meta.url,
          ),
          "utf8",
        ),
      ) as Record<string, string>;
      const page = await context.newPage();
      await page.goto(`/${locale}/login?return_path=friends&code=ABCD2345`);
      await expect(page.locator('meta[name="referrer"]')).toHaveAttribute(
        "content",
        "no-referrer",
      );
      await page
        .getByRole("button", {
          name: messages["auth.continue"].replace(
            "{{provider}}",
            messages[`auth.provider.${providers[index % 4].toLowerCase()}`],
          ),
        })
        .click();
      await expect(page).toHaveURL(
        `${origin}/${locale}/onboarding?return_path=friends&code=ABCD2345`,
      );
      await page.getByRole("textbox").fill(`Guest${index}`);
      await page
        .getByRole("button", {
          name: messages[index % 2 ? "auth.tutorial" : "auth.skip"],
        })
        .click();
      if (index % 2) {
        await expect(page).toHaveURL(new RegExp(`/${locale}/tutorial\\?`));
        await expect
          .poll(() => new URL(page.url()).searchParams.get("code"))
          .toBe("ABCD2345");
        await page
          .getByRole("button", { name: messages["tutorial.skip"] })
          .click();
      }
      await expect(page).toHaveURL(`${origin}/${locale}/friends?code=ABCD2345`);
      expect(
        await page.evaluate(() => localStorage.length + sessionStorage.length),
      ).toBe(0);
      const account = (
        await page.request.get("/api/v1/auth/bootstrap").then((r) => r.json())
      ).account;
      expect(account.nickname).toBe(`Guest${index}`);
    } finally {
      await context.close();
    }
  });
}

test("cancelled invitation returns the consumed transaction for a fresh safe login retry", async ({
  page,
  context,
}) => {
  await approve(context, true);
  await page.goto("/en/login?return_path=friends&code=ZZZZ6789");
  await page.getByRole("button", { name: "Continue with Google" }).click();
  await expect(page).toHaveURL(
    `${origin}/en/login?error=auth_failed&return_path=friends&code=ZZZZ6789`,
  );
  await expect(page.getByRole("alert")).toBeVisible();
  await approve(context);
  await page.getByRole("button", { name: "Continue with Google" }).click();
  await expect(page).toHaveURL(
    `${origin}/en/onboarding?return_path=friends&code=ZZZZ6789`,
  );
  await page.getByRole("textbox").fill("Invited");
  await page.getByRole("button", { name: "Save and skip tutorial" }).click();
  await expect(page).toHaveURL(`${origin}/en/friends?code=ZZZZ6789`);
});
async function approve(context: BrowserContext, cancel = false) {
  const subject = randomUUID();
  await context.route(
    /^https:\/\/(accounts\.google\.com|appleid\.apple\.com|kauth\.kakao\.com|nid\.naver\.com)\//,
    async (route) => {
      const url = new URL(route.request().url());
      expect(url.searchParams.has("invite_code")).toBe(false);
      expect(url.searchParams.has("return_path")).toBe(false);
      expect(route.request().headers().referer).toBeUndefined();
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
async function login(page: Page, provider = "Google", capture?: number) {
  await page.goto("/en/login");
  if (capture) {
    await mkdir(".tmp/auth-layout", { recursive: true });
    await page.screenshot({
      path: `.tmp/auth-layout/login-${capture}.png`,
      fullPage: true,
    });
  }
  await page.getByRole("button", { name: `Continue with ${provider}` }).click();
  await expect(page).toHaveURL(/\/en\/onboarding\?/);
  await page.getByRole("textbox", { name: "Nickname" }).fill("e\u0301探偵");
  if (capture)
    await page.screenshot({
      path: `.tmp/auth-layout/onboarding-${capture}.png`,
      fullPage: true,
    });
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
        await login(
          page,
          provider,
          provider === "Google" ? viewport.width : undefined,
        );
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
          await expect
            .poll(() =>
              page.evaluate(
                async () =>
                  (await navigator.serviceWorker.getRegistration("/"))?.active
                    ?.state,
              ),
            )
            .toBe("activated");
          const cachesUsed = await page.evaluate(async () => {
            const urls: string[] = [];
            for (const name of await caches.keys())
              for (const req of await (await caches.open(name)).keys())
                urls.push(req.url);
            return urls;
          });
          expect(
            cachesUsed.some((url) => new URL(url).pathname.startsWith("/api/")),
          ).toBe(false);
          const stored = await page.evaluate(async () => {
            const values: unknown[] = [];
            for (const info of await indexedDB.databases()) {
              if (!info.name) continue;
              const db = await new Promise<IDBDatabase>((resolve, reject) => {
                const open = indexedDB.open(info.name!);
                open.onsuccess = () => resolve(open.result);
                open.onerror = () => reject(open.error);
              });
              try {
                for (const store of db.objectStoreNames)
                  values.push(
                    await new Promise((resolve, reject) => {
                      const get = db
                        .transaction(store)
                        .objectStore(store)
                        .getAll();
                      get.onsuccess = () => resolve(get.result);
                      get.onerror = () => reject(get.error);
                    }),
                  );
              } finally {
                db.close();
              }
            }
            return JSON.stringify(values);
          });
          expect(stored).not.toMatch(
            /credential|csrf|subject|fixture-|__Host-liar/,
          );
          expect(stored).not.toContain(account.id);
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
test("export, linking, last-provider guard and unlink session revocation use production routes", async ({
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

test("signing out invalidates a second tab without persisting account credentials", async ({
  page,
  context,
}) => {
  await approve(context);
  await login(page);
  const peer = await context.newPage();
  await peer.goto("/en/settings");
  await expect(peer.getByText("é探偵", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Sign out" }).click();
  await expect(peer.locator("a.account-card")).toBeVisible();
  await expect(page.locator("a.account-card")).toBeVisible();
  expect(
    (await context.cookies()).some((c) => c.name === "__Host-liar_session"),
  ).toBe(false);
});

test("a cookie account switch cannot delete the account named in a stale confirmation or the new account", async ({
  page,
  context,
  browser,
}) => {
  await approve(context);
  await login(page);
  await page.getByRole("button", { name: "Delete account" }).click();
  const other = await browser.newContext({
    baseURL: origin,
    ignoreHTTPSErrors: true,
  });
  try {
    await approve(other);
    const otherPage = await other.newPage();
    await login(otherPage);
    const accountB = (
      await (await otherPage.request.get("/api/v1/auth/bootstrap")).json()
    ).account;
    await context.addCookies([
      (await other.cookies()).find((c) => c.name === "__Host-liar_session")!,
    ]);
    let deletions = 0;
    page.on("request", (req) => {
      if (
        req.method() === "DELETE" &&
        new URL(req.url()).pathname === "/api/v1/me"
      )
        deletions++;
    });
    await page
      .getByRole("button", { name: "Delete account permanently" })
      .click();
    await expect(
      page.getByRole("button", { name: "Delete account permanently" }),
    ).toBeDisabled();
    expect(deletions).toBe(0);
    expect(
      (await (await otherPage.request.get("/api/v1/auth/bootstrap")).json())
        .account.id,
    ).toBe(accountB.id);
  } finally {
    await other.close();
  }
});

test("the four configured login methods and private-information notice fit every locale on desktop and mobile", async ({
  browser,
}) => {
  for (const viewport of [
    { width: 1440, height: 900 },
    { width: 390, height: 844 },
  ]) {
    const context = await browser.newContext({
      baseURL: origin,
      viewport,
      ignoreHTTPSErrors: true,
    });
    try {
      const page = await context.newPage();
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
        await page.goto(`/${locale}/login`);
        await expect(page.locator("html")).toHaveAttribute("lang", locale);
        await expect(page.locator(".provider-button")).toHaveCount(4);
        for (const button of await page.locator(".provider-button").all())
          await expect(button).toBeEnabled();
        await expect(page.locator(".privacy-card")).toBeVisible();
        expect(
          await page.evaluate(
            () => document.documentElement.scrollWidth <= innerWidth,
          ),
        ).toBe(true);
      }
    } finally {
      await context.close();
    }
  }
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
