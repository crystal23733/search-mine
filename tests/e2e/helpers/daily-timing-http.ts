import { test, expect, type BrowserContext, type Page } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { advance } from "./online-quick";
import { solveDaily } from "./daily-http";
import type {
  AuthBootstrap,
  DailyStartResponse,
  DailyRankingsResponse,
  MeHistory,
} from "../../../packages/protocol/src/index";
type Approve = (context: BrowserContext) => Promise<void>;
type Login = (page: Page) => Promise<void>;

export function registerDailyTimingHttpTests(approve: Approve, login: Login) {
  for (const width of [1440, 390])
    test(`daily timing HTTPS preserves server time and owner history at ${width}px`, async ({
      browser,
    }) => {
      const context = await browser.newContext({
        baseURL: "https://localhost:8443",
        ignoreHTTPSErrors: true,
        viewport: { width, height: 900 },
      });
      try {
        await approve(context);
        const page = await context.newPage();
        await login(page);
        const bootstrap = (await page.request
          .get("/api/v1/auth/bootstrap")
          .then((r) => r.json())) as AuthBootstrap;
        const identity = {
          v: 1,
          account_id: bootstrap.account!.id,
          session_revision: bootstrap.session_revision!,
        };
        const post = async (path: string, body: unknown) => {
          const reply = await page.evaluate(
            async ({ path, body, csrf }) => {
              const response = await fetch(`/api/v1/daily/${path}`, {
                method: "POST",
                credentials: "same-origin",
                headers: {
                  "Content-Type": "application/json",
                  "x-liar-csrf": csrf!,
                },
                body: JSON.stringify(body),
              });
              return {
                status: response.status,
                cache: response.headers.get("cache-control"),
                body: await response.json(),
              };
            },
            { path, body, csrf: bootstrap.csrf },
          );
          expect(reply.status).toBe(200);
          expect(reply.cache).toBe("no-store");
          return reply.body;
        };
        const attempt = crypto.randomUUID();
        const started = (await post("start", {
          ...identity,
          attempt_id: attempt,
        })) as DailyStartResponse;
        expect(started.heartbeat_interval_ms).toBe(15000);
        expect(
          await post("start", { ...identity, attempt_id: attempt }),
        ).toEqual(started);
        const replay = await solveDaily(page, started.metadata.date);
        expect(replay.metadata).toEqual(started.metadata);
        // Only the trusted fixture clock advances; replay ticks never become official time.
        const target = replay.final_time_ms + 10000;
        let elapsed = 0;
        let sequence = 0;
        while (elapsed < target) {
          const seconds = Math.min(15, Math.ceil((target - elapsed) / 1000));
          await advance(page, seconds);
          elapsed += seconds * 1000;
          const input = {
            ...identity,
            ticket_id: started.ticket_id,
            sequence: ++sequence,
          };
          const ack = await post("heartbeat", input);
          expect(ack.ack).toEqual({ sequence, elapsed_ms: elapsed });
          expect(await post("heartbeat", input)).toEqual(ack);
        }
        const input = {
          ...identity,
          attempt_id: attempt,
          ticket_id: started.ticket_id,
          replay,
        };
        const saved = await post("submit-timed", input);
        expect(saved.trust).toBe("verified_time");
        expect(saved.receipt.verified_elapsed_ms).toBe(
          elapsed - started.rules.rules.countdown_ms,
        );
        expect(saved.receipt.verified_elapsed_ms).toBeGreaterThan(
          replay.final_time_ms - started.rules.rules.countdown_ms,
        );
        await advance(page, 1);
        expect(
          await post("submit-timed", { ...input, ticket_id: null }),
        ).toEqual(saved);
        const own = await post("own", {
          ...identity,
          metadata: replay.metadata,
        });
        expect(own.receipt).toEqual(saved.receipt);
        const publicRankings = () =>
          page.evaluate(
            async (query) => {
              const response = await fetch("/api/v1/daily/rankings", {
                method: "POST",
                credentials: "omit",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify(query),
              });
              return {
                status: response.status,
                cache: response.headers.get("cache-control"),
                body: await response.json(),
              };
            },
            {
              v: 1,
              date: started.metadata.date,
              seed_version: started.metadata.seed_version,
            },
          );
        const ranked = await publicRankings();
        expect(ranked.status).toBe(200);
        expect(ranked.cache).toBe("no-store");
        const rankings = ranked.body as DailyRankingsResponse;
        expect(rankings.metadata).toEqual(started.metadata);
        expect(rankings.time.entries).toContainEqual({
          position: expect.any(Number),
          nickname: bootstrap.account!.nickname,
          mistakes: saved.receipt.mistakes,
          elapsed_ms: saved.receipt.verified_elapsed_ms,
        });
        expect(rankings.completion.entries).toContainEqual({
          position: expect.any(Number),
          nickname: bootstrap.account!.nickname,
          mistakes: saved.receipt.mistakes,
        });
        expect(JSON.stringify(rankings)).not.toContain(identity.account_id);
        expect(JSON.stringify(rankings)).not.toContain(attempt);
        expect(JSON.stringify(rankings)).not.toContain(
          saved.receipt.completion_id,
        );
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
          await advance(page, 4);
          const messages = JSON.parse(
            await readFile(
              new URL(
                `../../../apps/web/src/locales/${locale}/common.json`,
                import.meta.url,
              ),
              "utf8",
            ),
          ) as Record<string, string>;
          await page.goto(`/${locale}/me/history`);
          const waiting = page.waitForResponse(
            (r) =>
              new URL(r.url()).pathname === "/api/v1/me/history" &&
              r.request().postDataJSON()?.kind === "daily",
          );
          await page
            .getByRole("combobox", { name: messages["me.kind"], exact: true })
            .selectOption("daily");
          const response = await waiting;
          expect(response.status()).toBe(200);
          expect(response.headers()["cache-control"]).toBe("no-store");
          const value = (await response.json()) as MeHistory;
          expect(value.total).toBe(1);
          expect(value.items).toHaveLength(1);
          expect(value.items[0]).toMatchObject({
            kind: "daily",
            entry: {
              completion_id: saved.receipt.completion_id,
              verified_elapsed_ms: saved.receipt.verified_elapsed_ms,
              trust: "verified_time",
            },
          });
          await expect(page.getByTestId("me-record")).toHaveCount(1);
          await page.getByTestId("me-record").locator("summary").click();
          await expect(
            page.getByText(messages["me.serverTime"], { exact: true }),
          ).toBeVisible();
          expect(JSON.stringify(value)).not.toMatch(
            /replay_json|replay_digest|"replay":|attempt_id|rules_snapshot/,
          );
        }
        await page.goto("/ko/me/account");
        await page
          .getByRole("button", { name: "로그아웃", exact: true })
          .click();
        await expect(
          page.getByText("로그아웃했어요.", { exact: true }),
        ).toBeVisible();
        const stale = await page.evaluate(
          async ({ input, csrf }) =>
            fetch("/api/v1/daily/submit-timed", {
              method: "POST",
              headers: {
                "Content-Type": "application/json",
                "x-liar-csrf": csrf!,
              },
              body: JSON.stringify(input),
            }).then((r) => r.status),
          { input, csrf: bootstrap.csrf },
        );
        expect(stale).toBe(401);
        expect((await publicRankings()).status).toBe(200);
      } finally {
        await context.close();
      }
    });
}
