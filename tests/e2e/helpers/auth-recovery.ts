import { test, expect, type BrowserContext, type Page } from "@playwright/test";

type Approve = (context: BrowserContext) => Promise<void>;
type Login = (page: Page) => Promise<void>;
const origin = "https://localhost:8443";

export function registerAuthRecoveryTests(approve: Approve, login: Login) {
  for (const width of [1440, 390])
    test(`offline authority is suspended and only the same live HTTPS session recovers at ${width}px`, async ({
      browser,
    }) => {
      const context = await browser.newContext({
        baseURL: origin,
        ignoreHTTPSErrors: true,
        viewport: { width, height: 900 },
      });
      let revoker: BrowserContext | undefined;
      try {
        await approve(context);
        const page = await context.newPage();
        await login(page);
        const cookie = (await context.cookies()).find(
          (c) => c.name === "__Host-liar_session",
        )!;
        const original = await (
          await context.request.get("/api/v1/auth/bootstrap")
        ).json();
        let writes = 0;
        page.on("request", (request) => {
          if (
            new URL(request.url()).pathname.startsWith("/api/") &&
            request.method() !== "GET"
          )
            writes++;
        });
        const download = page.getByRole("button", { name: "Download my data" });
        await context.setOffline(true);
        await expect(download).toHaveCount(0);
        await expect(
          page.getByText(
            "The account service could not respond. Retry when connected.",
            { exact: true },
          ),
        ).toBeVisible();
        expect(writes).toBe(0);
        await context.setOffline(false);
        await expect(download).toBeVisible();
        await expect(page.getByText("é探偵", { exact: true })).toBeVisible();
        const accepted = await (
          await context.request.get("/api/v1/auth/bootstrap")
        ).json();
        expect(accepted.account.id).toBe(original.account.id);
        expect(accepted.session_revision).toBe(original.session_revision);
        expect(
          (await context.cookies()).find((c) => c.name === cookie.name)!.value,
        ).toBe(cookie.value);
        expect(writes).toBe(0);
        expect(
          await page.evaluate(
            () => document.documentElement.scrollWidth <= innerWidth,
          ),
        ).toBe(true);
        const stored = await page.evaluate(() => ({
          local: { ...localStorage },
          session: { ...sessionStorage },
        }));
        expect(JSON.stringify(stored)).not.toContain(original.account.id);
        expect(JSON.stringify(stored)).not.toContain(original.session_revision);
        expect(JSON.stringify(stored)).not.toContain(original.csrf);

        revoker = await browser.newContext({
          baseURL: origin,
          ignoreHTTPSErrors: true,
          storageState: await context.storageState(),
        });
        await context.setOffline(true);
        await expect(download).toHaveCount(0);
        const proof = await (
          await revoker.request.get("/api/v1/auth/bootstrap")
        ).json();
        const logout = await revoker.request.post("/api/v1/auth/logout", {
          headers: { Origin: origin, "x-liar-csrf": proof.csrf },
          data: {},
        });
        expect(logout.status()).toBe(204);
        let delivered!: () => void, failed!: (error: unknown) => void;
        const rejectedProof = new Promise<void>((resolve, reject) => {
          delivered = resolve;
          failed = reject;
        });
        await page.route(
          "**/api/v1/auth/bootstrap",
          async (route) => {
            try {
              const response = await route.fetch({
                maxRedirects: 0,
                maxRetries: 0,
                timeout: 10000,
              });
              expect(response.status()).toBe(200);
              expect(response.headers()["cache-control"]).toBe("no-store");
              const proof = await response.json();
              expect(proof.account).toBeNull();
              expect(proof.session_revision).toBeNull();
              // Observe the real body before recovery discards and aborts its request.
              await route.fulfill({ response });
              delivered();
            } catch (error) {
              failed(error);
              await route.abort();
            }
          },
          { times: 1 },
        );
        await Promise.all([context.setOffline(false), rejectedProof]);
        await expect(
          page.getByText(
            "Check the input or refresh your account status before trying again.",
            { exact: true },
          ),
        ).toBeVisible();
        await expect(download).toHaveCount(0);
        expect(
          (await (await context.request.get("/api/v1/auth/bootstrap")).json())
            .account,
        ).toBeNull();
        expect(writes).toBe(0);
      } finally {
        await revoker?.close();
        await context.close();
      }
    });
}
