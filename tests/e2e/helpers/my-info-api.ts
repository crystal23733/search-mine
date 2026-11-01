import { test, expect, type BrowserContext, type Page } from "@playwright/test";
import type {
  AuthBootstrap,
  MeOverview,
  MeHistory,
} from "../../../packages/protocol/src/index";
import { advance } from "./online-quick";
type Approve = (context: BrowserContext) => Promise<void>;
type Login = (page: Page) => Promise<void>;
export function registerMyInfoApiTests(approve: Approve, login: Login) {
  test("my info reads own stored data through actual HTTPS without recent reauthentication", async ({
    browser,
  }) => {
    const context = await browser.newContext({
      baseURL: "https://localhost:8443",
      ignoreHTTPSErrors: true,
    });
    try {
      await approve(context);
      const page = await context.newPage();
      await login(page);
      await advance(page, 301);
      const bootstrap = (await page.request
        .get("/api/v1/auth/bootstrap")
        .then((r) => r.json())) as AuthBootstrap;
      const result = await page.evaluate(async (bootstrap) => {
        const post = async (path: string, body: unknown) => {
          const response = await fetch(`/api/v1/me/${path}`, {
            method: "POST",
            credentials: "same-origin",
            headers: {
              "Content-Type": "application/json",
              "x-liar-csrf": bootstrap.csrf!,
            },
            body: JSON.stringify(body),
          });
          return {
            status: response.status,
            cache: response.headers.get("cache-control"),
            body: await response.json(),
          };
        };
        return {
          overview: await post("overview", { v: 1 }),
          online: await post("history", {
            v: 1,
            kind: "online",
            opponent: "all",
          }),
          daily: await post("history", {
            v: 1,
            kind: "daily",
            opponent: "all",
          }),
          injection: await post("overview", {
            v: 1,
            account_id: crypto.randomUUID(),
          }),
          filter: await post("history", {
            v: 1,
            kind: "daily",
            opponent: "bot",
          }),
        };
      }, bootstrap);
      for (const name of ["overview", "online", "daily"] as const) {
        expect(result[name].status).toBe(200);
        expect(result[name].cache).toBe("no-store");
      }
      const overview = result.overview.body as MeOverview;
      expect(overview.account.id).toBe(bootstrap.account!.id);
      expect(overview.online.played).toBe(0);
      expect(overview.online.win_rate).toBeNull();
      expect(overview.consent.cmp_available).toBe(false);
      expect(overview.consent.status).toBe("unknown");
      for (const name of ["online", "daily"] as const) {
        const page = result[name].body as MeHistory;
        expect(page.account_id).toBe(bootstrap.account!.id);
        expect(page.total).toBe(0);
        expect(page.items).toEqual([]);
        expect(page.next).toBeNull();
      }
      expect(result.injection.status).toBe(400);
      expect(result.filter.status).toBe(400);
      expect(JSON.stringify(result.overview.body)).not.toMatch(
        /seed|subject_digest|credential|token_hash|replay_json|replay_digest/,
      );
    } finally {
      await context.close();
    }
  });
}
