import { expect, test, type BrowserContext } from "@playwright/test";
import type {
  AuthBootstrap,
  ConsentSnapshot,
} from "../../../packages/protocol/src/index";

export function registerConsentApiTests(
  approve: (context: BrowserContext) => Promise<void>,
) {
  test("current consent control uses the real session before nickname setup and remains denied without CMP", async ({
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
      await page.goto("/en/login?return_path=me");
      await page.getByRole("button", { name: "Continue with Google" }).click();
      await expect(page).toHaveURL(/\/en\/onboarding\?return_path=me/);
      await page.goto("/en/me/privacy");
      await expect(page.getByTestId("me-server-consent")).toBeVisible();
      const proof = (await page.request
        .get("/api/v1/auth/bootstrap")
        .then((r) => r.json())) as AuthBootstrap;
      expect(proof.account?.nickname).toBeNull();
      const control = await page.evaluate(async (csrf) => {
        const post = async (path: string, body: unknown, proof = csrf) => {
          const response = await fetch(`/api/v1/me/consent${path}`, {
            method: "POST",
            credentials: "same-origin",
            cache: "no-store",
            redirect: "error",
            headers: {
              "content-type": "application/json",
              "x-liar-csrf": proof!,
            },
            body: JSON.stringify(body),
          });
          return {
            status: response.status,
            cache: response.headers.get("cache-control"),
            body: await response.json(),
          };
        };
        const initial = await post("", { v: 1 });
        const forged = await post("/refresh", {
          v: 1,
          expected_revision: null,
          analytics: true,
        });
        const wrongVersion = await post("", { v: 2 });
        const unavailable = await post("/refresh", {
          v: 1,
          expected_revision: null,
        });
        const analytics = await post("/withdraw", {
          v: 1,
          purpose: "analytics",
        });
        const advertising = await post("/withdraw", {
          v: 1,
          purpose: "advertising",
        });
        const all = await post("/withdraw", { v: 1, purpose: "all" });
        const stale = await post("/refresh", {
          v: 1,
          expected_revision: analytics.body.revision,
        });
        const forbidden = await post(
          "/withdraw",
          { v: 1, purpose: "all" },
          "invalid",
        );
        const current = await post("", { v: 1 });
        return {
          initial,
          forged,
          wrongVersion,
          unavailable,
          analytics,
          advertising,
          all,
          stale,
          forbidden,
          current,
        };
      }, proof.csrf);
      expect(control.initial.status).toBe(200);
      expect(control.initial.body.revision).toBeNull();
      expect(control.initial.body.consent.status).toBe("unknown");
      expect(control.forged.status).toBe(400);
      expect(control.wrongVersion.status).toBe(400);
      expect(control.unavailable.status).toBe(503);
      for (const result of [
        control.analytics,
        control.advertising,
        control.all,
        control.current,
      ]) {
        expect(result.status).toBe(200);
        expect(result.body.account_id).toBe(proof.account!.id);
        expect(result.body.consent).toMatchObject({
          cmp_available: false,
          status: "current",
          analytics: "denied",
          advertising: "denied",
        });
      }
      expect(control.analytics.body.revision).not.toBe(
        control.advertising.body.revision,
      );
      expect(control.all.body.revision).not.toBe(
        control.advertising.body.revision,
      );
      expect(control.current.body.revision).toBe(control.all.body.revision);
      expect(control.stale.status).toBe(409);
      expect(control.forbidden.status).toBe(403);
      for (const result of Object.values(control))
        expect(result.cache).toBe("no-store");
      expect(JSON.stringify(control.current.body)).not.toMatch(
        /subject|session_id|hash|credential|tc_string|csrf|email/,
      );
      expect(JSON.stringify(control.current.body)).not.toContain(
        proof.session_revision!,
      );

      await page.request.post("/__fixture/advance", { data: { seconds: 301 } });
      await page.goto("/en/me/privacy");
      await expect(page.getByTestId("me-server-consent")).toBeVisible();
      const later = await page.evaluate(async () => {
        const proof = await fetch("/api/v1/auth/bootstrap", {
          cache: "no-store",
        }).then((r) => r.json());
        const post = async (path: string) => {
          const response = await fetch(path, {
            method: "POST",
            credentials: "same-origin",
            cache: "no-store",
            headers: {
              "content-type": "application/json",
              "x-liar-csrf": proof.csrf,
            },
            body: JSON.stringify({ v: 1 }),
          });
          return { status: response.status, body: await response.json() };
        };
        return {
          current: await post("/api/v1/me/consent"),
          overview: await post("/api/v1/me/overview"),
        };
      });
      expect(later.current.status).toBe(200);
      expect(later.overview.status).toBe(200);
      expect(later.current.body.consent).toEqual(later.overview.body.consent);
      expect((later.current.body as ConsentSnapshot).revision).toBe(
        control.all.body.revision,
      );
      await page.goto("/en/me/account");
      await page.getByRole("button", { name: "Sign out", exact: true }).click();
      await expect(
        page.getByRole("link", { name: "Sign in", exact: true }),
      ).toBeVisible();
      const after = await page.evaluate(async () => {
        const proof = await fetch("/api/v1/auth/bootstrap", {
          cache: "no-store",
        }).then((r) => r.json());
        const response = await fetch("/api/v1/me/consent", {
          method: "POST",
          headers: {
            "content-type": "application/json",
            "x-liar-csrf": proof.csrf,
          },
          body: JSON.stringify({ v: 1 }),
        });
        return {
          account: proof.account,
          status: response.status,
          cache: response.headers.get("cache-control"),
        };
      });
      expect(after).toEqual({ account: null, status: 401, cache: "no-store" });
    } finally {
      await context.close();
    }
  });
}
