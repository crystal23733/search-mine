import { test, expect, type BrowserContext, type Page } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { solveDaily } from "./daily-http";
import { advance } from "./online-quick";
import type {
  AuthBootstrap,
  DailyStartResponse,
} from "../../../packages/protocol/src/index";

export function registerDailyRankingsTests(
  approve: (context: BrowserContext) => Promise<void>,
  login: (page: Page) => Promise<void>,
) {
  for (const width of [1440, 390])
    test(`anonymous daily rankings show both server lists in eight locales at ${width}px`, async ({
      browser,
    }) => {
      const publisher = await browser.newContext({
        baseURL: "https://localhost:8443",
        ignoreHTTPSErrors: true,
      });
      const reader = await browser.newContext({
        baseURL: "https://localhost:8443",
        ignoreHTTPSErrors: true,
        viewport: { width, height: 900 },
      });
      try {
        await approve(publisher);
        const author = await publisher.newPage();
        await login(author);
        const identity: AuthBootstrap = await publisher.request
          .get("/api/v1/auth/bootstrap")
          .then((r) => r.json());
        const headers = {
          origin: "https://localhost:8443",
          "x-liar-csrf": identity.csrf!,
        };
        const attempt = crypto.randomUUID();
        const body = {
          v: 1,
          account_id: identity.account!.id,
          session_revision: identity.session_revision!,
        };
        const started = await publisher.request.post("/api/v1/daily/start", {
          headers,
          data: { ...body, attempt_id: attempt },
        });
        expect(started.status()).toBe(200);
        const grant: DailyStartResponse = await started.json();
        const replay = await solveDaily(author, grant.metadata.date);
        let elapsed = 0,
          sequence = 0;
        while (elapsed < replay.final_time_ms + 10000) {
          const seconds = Math.min(
            15,
            Math.ceil((replay.final_time_ms + 10000 - elapsed) / 1000),
          );
          await advance(author, seconds);
          elapsed += seconds * 1000;
          const beat = await publisher.request.post("/api/v1/daily/heartbeat", {
            headers,
            data: { ...body, ticket_id: grant.ticket_id, sequence: ++sequence },
          });
          expect(beat.status()).toBe(200);
          expect((await beat.json()).ack).not.toBeNull();
        }
        const saved = await publisher.request.post(
          "/api/v1/daily/submit-timed",
          {
            headers,
            data: {
              ...body,
              attempt_id: attempt,
              ticket_id: grant.ticket_id,
              replay,
            },
          },
        );
        expect(saved.status()).toBe(200);
        expect((await saved.json()).trust).toBe("verified_time");
        const clock: { utc: number } = await publisher.request
          .get("/__fixture/clock")
          .then((r) => r.json());
        const page = await reader.newPage();
        await page.clock.setFixedTime(new Date(clock.utc * 1000));
        const queries: Array<Record<string, string>> = [];
        const writes: string[] = [];
        page.on("request", (request) => {
          if (request.url().endsWith("/api/v1/daily/rankings"))
            queries.push(request.headers());
          if (
            /\/api\/v1\/daily\/(start|heartbeat|submit|submit-timed)$/.test(
              new URL(request.url()).pathname,
            )
          )
            writes.push(request.url());
        });
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
          const messages: Record<string, string> = JSON.parse(
            await readFile(
              new URL(
                `../../../apps/web/src/locales/${locale}/common.json`,
                import.meta.url,
              ),
              "utf8",
            ),
          );
          await page.goto(`/${locale}/daily/rankings`);
          await expect(
            page.getByRole("heading", {
              level: 1,
              name: messages["ranking.title"] ?? "Daily rankings",
              exact: true,
            }),
          ).toBeVisible();
          const time = page.getByRole("table", {
            name: messages["ranking.time"],
          });
          const completion = page.getByRole("table", {
            name: messages["ranking.completion"],
          });
          await expect(
            time.getByRole("cell", { name: "é探偵", exact: true }).first(),
          ).toBeVisible();
          await expect(
            completion
              .getByRole("cell", { name: "é探偵", exact: true })
              .first(),
          ).toBeVisible();
          if (locale === "en") {
            await page.route("**/api/v1/daily/rankings", (route) =>
              route.fulfill({
                status: 503,
                contentType: "application/json",
                body: JSON.stringify({ code: "daily_unavailable" }),
              }),
            );
            await page
              .getByRole("button", { name: messages["ranking.refresh"] })
              .click();
            await expect(page.getByRole("alert")).toHaveText(
              messages["ranking.unavailable"],
            );
            await expect(page.getByRole("table")).toHaveCount(0);
            await page.unroute("**/api/v1/daily/rankings");
            await page
              .getByRole("button", { name: messages["ranking.refresh"] })
              .click();
            await expect(time).toBeVisible();
            await reader.setOffline(true);
            await expect(
              page
                .getByRole("status")
                .filter({ hasText: messages["ranking.offline"] }),
            ).toBeVisible();
            await expect(page.getByRole("table")).toHaveCount(0);
            await expect(
              page.getByRole("button", { name: messages["ranking.refresh"] }),
            ).toBeDisabled();
            await reader.setOffline(false);
            await expect(time).toBeVisible();
          }
          expect(
            await page.evaluate(
              () => document.documentElement.scrollWidth <= innerWidth,
            ),
          ).toBe(true);
        }
        expect(queries.length).toBeGreaterThanOrEqual(8);
        for (const query of queries) {
          expect(query.cookie).toBeUndefined();
          expect(query["x-liar-csrf"]).toBeUndefined();
        }
        expect(writes).toHaveLength(0);
        expect(
          (await reader.cookies()).filter(
            (c) => c.name === "__Host-liar_session",
          ),
        ).toHaveLength(0);
        const guest: AuthBootstrap = await reader.request
          .get("/api/v1/auth/bootstrap")
          .then((r) => r.json());
        expect(guest.account).toBeNull();
        expect(guest.session_revision).toBeNull();
      } finally {
        await reader.close();
        await publisher.close();
      }
    });
}
