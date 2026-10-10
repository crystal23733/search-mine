import { test, expect, type BrowserContext, type Page } from "@playwright/test";
import type { LatestResultResponse } from "../../../packages/protocol/src/index";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { advance, stream } from "./online-quick";

const origin = "https://localhost:8443";
type Approve = (context: BrowserContext) => Promise<void>;
type Login = (page: Page) => Promise<void>;

export function registerOnlineAbandonTests(approve: Approve, login: Login) {
  for (const width of [390, 1440])
    test(`actual two upstream disconnects persist abandoned draw for fresh browsers at ${width}px`, async ({
      browser,
      request,
    }) => {
      const contexts: BrowserContext[] = [];
      const context = async () => {
        const value = await browser.newContext({
          baseURL: origin,
          ignoreHTTPSErrors: true,
          viewport: { width, height: 900 },
        });
        contexts.push(value);
        return value;
      };
      try {
        const a = await context(),
          b = await context();
        await approve(a);
        await approve(b);
        const one = await a.newPage(),
          two = await b.newPage();
        await login(one);
        await login(two);
        const own = stream(one),
          peer = stream(two);
        await one.goto("/en/queue");
        await one.getByRole("button", { name: "Find an opponent" }).click();
        await expect(
          one.getByRole("button", { name: "Cancel waiting" }),
        ).toBeVisible();
        await two.goto("/en/queue");
        await two.getByRole("button", { name: "Find an opponent" }).click();
        await expect(one.getByRole("gridcell")).toHaveCount(9);
        await expect(two.getByRole("gridcell")).toHaveCount(9);
        await advance(one, 3);
        await expect.poll(() => own.lastView()?.phase).toBe("playing");
        await expect.poll(() => peer.lastView()?.phase).toBe("playing");
        await one.locator('[data-cell="8"]').click();
        await expect
          .poll(() => own.lastView()?.own.cells[8].state)
          .toBe("safe");
        const id = own.snapshot()!.match_id,
          hash = own.lastView()!.rules.hash;
        expect(peer.snapshot()!.match_id).toBe(id);
        const admitted =
          (await (await one.request.get("/__fixture/clock")).json()).utc - 3;
        const cookies = [await a.cookies(origin), await b.cookies(origin)];
        const transport = async () => {
          const response = await request.get(`${origin}/__fixture/transport`, {
            ignoreHTTPSErrors: true,
          });
          expect(response.status()).toBe(200);
          expect(response.headers()["cache-control"]).toBe("no-store");
          const value = await response.json();
          expect(Object.keys(value).sort()).toEqual([
            "accepted",
            "forced",
            "pending",
            "server_ended",
          ]);
          return value as {
            accepted: number;
            forced: number;
            pending: number;
            server_ended: number;
          };
        };
        const connected = await transport();
        expect(connected.pending).toBe(2);
        // Freeze the fixture clock until the server has closed both upgraded transports.
        await Promise.all([a.close(), b.close()]);
        await expect
          .poll(async () => {
            const value = await transport();
            expect(value.forced).toBe(connected.forced);
            return { pending: value.pending, server_ended: value.server_ended };
          })
          .toEqual({ pending: 0, server_ended: connected.server_ended + 2 });
        expect(
          (
            await (
              await request.get(`${origin}/__fixture/clock`, {
                ignoreHTTPSErrors: true,
              })
            ).json()
          ).utc,
        ).toBe(admitted + 3);
        expect(
          (
            await request.post(`${origin}/__fixture/advance`, {
              ignoreHTTPSErrors: true,
              data: { seconds: 30 },
            })
          ).ok(),
        ).toBe(true);
        await expect
          .poll(async () =>
            (
              await request.get(`${origin}/__fixture/persisted/${id}`, {
                ignoreHTTPSErrors: true,
              })
            ).status(),
          )
          .toBe(200);

        const bodies: LatestResultResponse[] = [];
        for (const [index, saved] of cookies.entries()) {
          const fresh = await context();
          await fresh.addCookies(saved);
          const page = await fresh.newPage();
          const urls: string[] = [],
            sockets: string[] = [];
          page.on("websocket", (socket) => sockets.push(socket.url()));
          page.on("request", (r) => {
            urls.push(r.url());
            if (new URL(r.url()).pathname === "/api/v1/results/latest")
              expect(r.postDataJSON()).toEqual({ v: 1 });
          });
          const response = page.waitForResponse(
            (r) =>
              new URL(r.url()).pathname === "/api/v1/results/latest" &&
              r.status() === 200,
          );
          await page.goto("/en/results");
          const reply = await response;
          expect(reply.headers()["cache-control"]).toBe("no-store");
          const body: LatestResultResponse = await reply.json();
          expect(body.result).toEqual({
            match_id: id,
            rules_hash: hash,
            end_elapsed_ms: 33000,
            result: { reason: "abandoned", outcome: "draw", completed: true },
            own: {
              opened_safe: index === 0 ? 5 : 4,
              mistakes: 0,
              accusation_attempts: 0,
              correct_accusations: 0,
            },
          });
          bodies.push(body);
          expect(JSON.stringify(body)).not.toMatch(
            /"seed"|"board"|"account"|"provider"|"session"|"opponent"|"csrf"/,
          );
          const result = page.getByTestId("personal-result");
          await expect(result).toBeVisible();
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
            const messages = JSON.parse(
              await readFile(
                new URL(
                  `../../../apps/web/src/locales/${locale}/common.json`,
                  import.meta.url,
                ),
                "utf8",
              ),
            );
            await page.getByRole("combobox").first().selectOption(locale);
            await expect(page.locator("html")).toHaveAttribute("lang", locale);
            await expect(result).toContainText(messages["result.savedTitle"]);
            await expect(result).not.toContainText(
              messages["result.statsUnavailable"],
            );
            expect(
              await page.evaluate(
                () => document.documentElement.scrollWidth <= innerWidth,
              ),
            ).toBe(true);
          }
          await expect(page.getByRole("grid")).toHaveCount(0);
          expect(sockets).toEqual([]);
          expect(
            urls.some(
              (url) =>
                url.includes(id) || new URL(url).pathname === "/api/v1/lobby",
            ),
          ).toBe(false);
          expect(
            await page.evaluate(() =>
              JSON.stringify({ ...localStorage, ...sessionStorage }),
            ),
          ).not.toContain(id);
          await mkdir(".tmp/online-layout", { recursive: true });
          await page.screenshot({
            path: `.tmp/online-layout/abandoned-${index}-${width}.png`,
            fullPage: true,
          });
        }
        expect(bodies[0].result?.result).toEqual(bodies[1].result?.result);
        expect(bodies[0].result?.own).not.toEqual(bodies[1].result?.own);
        const metadata = async () => {
          const response = await request.get(
            `${origin}/__fixture/persisted/${id}`,
            { ignoreHTTPSErrors: true },
          );
          expect(response.status()).toBe(200);
          return response.json();
        };
        const before = await metadata();
        expect(before).toMatchObject({
          active_count: 0,
          result_count: 1,
          retention_started_at: admitted,
          end_elapsed_ms: 33000,
          reason: "abandoned",
          unknown_players: 0,
        });
        expect(
          (
            await request.post(`${origin}/__fixture/advance`, {
              ignoreHTTPSErrors: true,
              data: { seconds: 240 },
            })
          ).ok(),
        ).toBe(true);
        const restarted = await request.post(
          "http://127.0.0.1:3002/__fixture/restart",
        );
        expect(restarted.status()).toBe(200);
        const process = await restarted.json();
        expect(process.killed).toBe(true);
        expect(process.before_pid).not.toBe(process.after_pid);
        expect(
          process.exit_signal === "SIGKILL" ||
            (typeof process.exit_code === "number" && process.exit_code !== 0),
        ).toBe(true);
        expect(process.clock).toBe(admitted + 273);
        expect(await metadata()).toEqual(before);
        for (const [index, saved] of cookies.entries()) {
          const fresh = await context();
          await fresh.addCookies(saved);
          const page = await fresh.newPage();
          const response = page.waitForResponse(
            (r) =>
              new URL(r.url()).pathname === "/api/v1/results/latest" &&
              r.status() === 200,
          );
          await page.goto("/en/results");
          expect(await (await response).json()).toEqual(bodies[index]);
          await expect(page.getByTestId("personal-result")).toBeVisible();
          const retry = page.waitForResponse(
            (r) =>
              new URL(r.url()).pathname === "/api/v1/results/latest" &&
              r.status() === 200,
          );
          await page.reload();
          expect(await (await retry).json()).toEqual(bodies[index]);
          await expect(page.getByTestId("personal-result")).toBeVisible();
        }
        expect(await metadata()).toEqual(before);
        await writeFile(
          `.tmp/online-layout/abandoned-${width}.json`,
          JSON.stringify(
            { transport: await transport(), process, metadata: before },
            null,
            2,
          ),
        );
      } finally {
        await Promise.all(contexts.map((value) => value.close()));
      }
    });
}
