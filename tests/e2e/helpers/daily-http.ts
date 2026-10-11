import { test, expect, type BrowserContext, type Page } from "@playwright/test";
import { readFile } from "node:fs/promises";
import type {
  AuthBootstrap,
  DailyReplay,
  DailyView,
  DailyStep,
  LocalInput,
  PersonalDataExport,
} from "../../../packages/protocol/src/index";
type Approve = (context: BrowserContext) => Promise<void>;
type Login = (page: Page) => Promise<void>;
export function registerDailyHttpTests(approve: Approve, login: Login) {
  for (const width of [1440, 390])
    test(`authenticated daily HTTP verifies actual WASM completion and exports only its owner at ${width}px`, async ({
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
        const replay = await page.evaluate(async (date) => {
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
                JSON.parse(
                  session.step(JSON.stringify(input), time),
                ) as DailyStep
              ).view;
            }
            if (!view.completed) throw Error("WASM attempt did not complete");
            return JSON.parse(session.replay()) as DailyReplay;
          } finally {
            session.free();
          }
        }, date);
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
        const download = page.waitForEvent("download");
        await page.getByRole("button", { name: "Download my data" }).click();
        const saved = await download;
        const path = await saved.path();
        expect(path).not.toBeNull();
        const exported = JSON.parse(
          await readFile(path!, "utf8"),
        ) as PersonalDataExport;
        expect(exported.games).toHaveLength(1);
        expect(exported.games[0]).toMatchObject({
          kind: "daily",
          completion_id: result.first.body.receipt.completion_id,
          metadata: replay.metadata,
          replayed_elapsed_ms: result.first.body.receipt.replayed_elapsed_ms,
        });
        expect(exported.replays).toHaveLength(1);
        expect(exported.replays[0].replay).toEqual(replay);
      } finally {
        await context.close();
      }
    });
}
