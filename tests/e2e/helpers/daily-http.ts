import { test, expect, type BrowserContext, type Page } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { advance } from "./online-quick";
import type {
  AuthBootstrap,
  DailyReplay,
  DailyView,
  DailyStep,
  LocalInput,
  MeHistory,
} from "../../../packages/protocol/src/index";
type Approve = (context: BrowserContext) => Promise<void>;
type Login = (page: Page) => Promise<void>;
export function registerDailyHttpTests(approve: Approve, login: Login) {
  for (const width of [1440, 390])
    test(`authenticated daily HTTP verifies actual WASM completion and shows only its owner’s history at ${width}px`, async ({
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
        const utc = (await page.request
          .get("/__fixture/clock")
          .then((r) => r.json())) as { utc: number };
        const date = new Date(utc.utc * 1000).toISOString().slice(0, 10);
        const replay = await solveDaily(page, date);
        const result = await page.evaluate(
          async ({ bootstrap, date, replay }) => {
            const identity = {
              v: 1,
              account_id: bootstrap.account!.id,
              session_revision: bootstrap.session_revision!,
            };
            const post = async (path: string, body: unknown) => {
              const response = await fetch(`/api/v1/daily/${path}`, {
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
            const definition = await post("definition", {
              ...identity,
              date,
              seed_version: 1,
            });
            const empty = await post("own", {
              ...identity,
              metadata: replay.metadata,
            });
            const submission = {
              ...identity,
              attempt_id: crypto.randomUUID(),
              replay,
            };
            const first = await post("submit", submission);
            const retry = await post("submit", submission);
            const practice = await post("submit", {
              ...submission,
              attempt_id: crypto.randomUUID(),
            });
            const own = await post("own", {
              ...identity,
              metadata: replay.metadata,
            });
            const foreign = await post("own", {
              ...identity,
              account_id: crypto.randomUUID(),
              metadata: replay.metadata,
            });
            return { definition, empty, first, retry, practice, own, foreign };
          },
          { bootstrap, date, replay },
        );
        for (const name of [
          "definition",
          "empty",
          "first",
          "retry",
          "practice",
          "own",
        ] as const) {
          expect(result[name].status).toBe(200);
          expect(result[name].cache).toBe("no-store");
        }
        expect(result.definition.body.metadata).toEqual(replay.metadata);
        expect(result.empty.body.receipt).toBeNull();
        expect(result.first.body).toMatchObject({
          account_id: bootstrap.account!.id,
          trust: "verified_completion",
          disposition: "official",
        });
        expect(result.retry.body).toEqual(result.first.body);
        expect(result.practice.body).toMatchObject({
          trust: "verified_completion",
          disposition: "practice",
          receipt: result.first.body.receipt,
        });
        expect(result.practice.body.attempt_id).not.toBe(
          result.practice.body.receipt.attempt_id,
        );
        expect(result.own.body.receipt).toEqual(result.first.body.receipt);
        expect(result.foreign.status).toBe(401);
        expect(JSON.stringify(result.first.body)).not.toMatch(
          /subject|credential|nickname|email|verified_time|official_elapsed_ms/,
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
              completion_id: result.first.body.receipt.completion_id,
              verified_elapsed_ms: null,
              trust: "verified_completion",
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
      } finally {
        await context.close();
      }
    });
}

export async function solveDaily(
  page: Page,
  date: string,
): Promise<DailyReplay> {
  return page.evaluate(async (date) => {
    const uri = "/core/liar_wasm.js";
    const wasm = await import(/* @vite-ignore */ uri);
    await wasm.default();
    const session = new wasm.DailySession(date, 1);
    try {
      let time = 3000;
      let view = JSON.parse(session.advance(time)) as DailyView;
      let sequence = 0;
      for (
        let cell = 0;
        cell < view.own.cells.length && !view.completed;
        cell++
      ) {
        if (view.own.cells[cell].state !== "closed") continue;
        time += Math.max(50, view.own.stun_ms);
        const input: LocalInput = {
          v: 1,
          command_id: ++sequence,
          client_seq: sequence,
          action: { type: "open", cell },
        };
        view = (
          JSON.parse(session.step(JSON.stringify(input), time)) as DailyStep
        ).view;
      }
      if (!view.completed) throw Error("WASM attempt did not complete");
      return JSON.parse(session.replay()) as DailyReplay;
    } finally {
      session.free();
    }
  }, date);
}
