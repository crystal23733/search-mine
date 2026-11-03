import { expect, test, type BrowserContext } from "@playwright/test";
import type {
  AuthBootstrap,
  OptionalEventCaptureInput,
} from "../../../packages/protocol/src/index";

export function registerOptionalEventApiTests(
  approve: (context: BrowserContext) => Promise<void>,
) {
  test("optional event API rejects collection without current CMP permission over real HTTPS", async ({
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
      const automatic: string[] = [];
      page.on("request", (request) => {
        if (request.url().includes("/api/v1/events/optional"))
          automatic.push(request.url());
      });
      await page.goto("/en/login?return_path=me");
      await page.getByRole("button", { name: "Continue with Google" }).click();
      await expect(page).toHaveURL(/\/en\/onboarding\?return_path=me/);
      await page.goto("/en/me/privacy");
      await expect(page.getByTestId("me-server-consent")).toBeVisible();
      expect(automatic).toEqual([]);
      const proof = (await page.request
        .get("/api/v1/auth/bootstrap")
        .then((r) => r.json())) as AuthBootstrap;
      expect(proof.account?.nickname).toBeNull();
      if (!proof.csrf)
        throw new Error("Authenticated fixture must provide CSRF");
      const result = await page.evaluate(async (csrf) => {
        const post = async (path: string, input: unknown, proof = csrf) => {
          const response = await fetch(path, {
            method: "POST",
            credentials: "same-origin",
            cache: "no-store",
            redirect: "error",
            headers: {
              "content-type": "application/json",
              "x-liar-csrf": proof,
            },
            body: JSON.stringify(input),
          });
          return {
            status: response.status,
            cache: response.headers.get("cache-control"),
            body: await response.json(),
          };
        };
        const time = await fetch("/__fixture/clock").then((r) => r.json());
        const input: OptionalEventCaptureInput = {
          v: 1,
          expected_revision: crypto.randomUUID(),
          draft: {
            event_version: 1,
            event_id: crypto.randomUUID(),
            session_key: crypto.randomUUID(),
            occurred_at: time.utc,
            locale: "en",
            event: { type: "landing_view", route: "home" },
          },
        };
        const unknown = await post("/api/v1/events/optional", input);
        const unavailable = await post("/api/v1/me/consent/refresh", {
          v: 1,
          expected_revision: null,
        });
        await post("/api/v1/me/consent/withdraw", { v: 1, purpose: "all" });
        const current = await post("/api/v1/me/consent", { v: 1 });
        input.expected_revision = current.body.revision;
        return {
          unknown,
          unavailable,
          denied: await post("/api/v1/events/optional", input),
          forged: await post("/api/v1/events/optional", {
            ...input,
            analytics: true,
          }),
          forbidden: await post("/api/v1/events/optional", input, "wrong"),
        };
      }, proof.csrf);
      expect(result.unknown.status).toBe(403);
      expect(result.denied.status).toBe(403);
      expect(result.unknown.body).toEqual({ error: "consent" });
      expect(result.denied.body).toEqual({ error: "consent" });
      expect(result.unavailable.status).toBe(503);
      expect(result.forged.status).toBe(400);
      expect(result.forbidden.status).toBe(403);
      for (const response of Object.values(result))
        expect(response.cache).toBe("no-store");
      await page.goto("/en/me/account");
      await page.getByRole("button", { name: "Sign out", exact: true }).click();
      await expect(
        page.getByRole("link", { name: "Sign in", exact: true }),
      ).toBeVisible();
      const after = await page.evaluate(async () => {
        const proof = await fetch("/api/v1/auth/bootstrap", {
          cache: "no-store",
        }).then((r) => r.json());
        const response = await fetch("/api/v1/events/optional", {
          method: "POST",
          headers: {
            "content-type": "application/json",
            "x-liar-csrf": proof.csrf,
          },
          body: JSON.stringify({ v: 1 }),
        });
        return {
          status: response.status,
          cache: response.headers.get("cache-control"),
        };
      });
      expect(after).toEqual({ status: 401, cache: "no-store" });
    } finally {
      await context.close();
    }
  });
}
