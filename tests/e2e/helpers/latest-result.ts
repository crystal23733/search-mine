import { test, expect, type BrowserContext, type Page } from "@playwright/test";
import type { LatestResultResponse } from "../../../packages/protocol/src/index";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { advance, stream } from "./online-quick";
const origin = "https://localhost:8443";
type Approve = (context: BrowserContext) => Promise<void>;
type Login = (page: Page) => Promise<void>;
export function registerLatestResultTests(approve: Approve, login: Login) {
  for (const width of [390, 1440])
    for (const known of [false, true])
      test(`fresh browser discovers ${known ? "known" : "unknown"} latest result after actual OS kill/restart at ${width}px`, async ({
        browser,
        request,
      }) => {
        const a = await browser.newContext({
            baseURL: origin,
            ignoreHTTPSErrors: true,
          }),
          b = await browser.newContext({
            baseURL: origin,
            ignoreHTTPSErrors: true,
          });
        let fresh: BrowserContext | undefined,
          other: BrowserContext | undefined;
        try {
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
          await expect
            .poll(() => Boolean(own.snapshot() && peer.snapshot()))
            .toBe(true);
          const id = own.snapshot()!.match_id,
            hash = own.lastView()!.rules.hash;
          const admitted = (
            await (await one.request.get("/__fixture/clock")).json()
          ).utc;
          await advance(one, 3);
          await expect.poll(() => own.lastView()?.phase).toBe("playing");
          if (known) {
            await advance(two, 240);
            await expect(two.getByTestId("recording")).toHaveText(
              "Result saved.",
            );
          }
          const cookies = await a.cookies(origin);
          const restarted = await request.post(
            "http://127.0.0.1:3002/__fixture/restart",
          );
          expect(restarted.status()).toBe(200);
          const process = await restarted.json();
          expect(process.before_pid).not.toBe(process.after_pid);
          expect(process.killed).toBe(true);
          expect(
            process.exit_signal === "SIGKILL" ||
              (typeof process.exit_code === "number" &&
                process.exit_code !== 0),
          ).toBe(true);
          expect(process.ready).toBe(true);
          expect(process.clock).toBe(admitted + (known ? 243 : 3));
          await a.close();
          await b.close();
          fresh = await browser.newContext({
            baseURL: origin,
            ignoreHTTPSErrors: true,
            viewport: { width, height: 900 },
          });
          await fresh.addCookies(cookies);
          const page = await fresh.newPage();
          const requested: string[] = [];
          page.on("request", (r) => {
            requested.push(r.url());
            if (new URL(r.url()).pathname === "/api/v1/results/latest")
              expect(r.postDataJSON()).toEqual({ v: 1 });
          });
          // A new page receives only authentication cookies; no old URL, match ID, board or storage.
          const reply = page.waitForResponse(
            (r) =>
              new URL(r.url()).pathname === "/api/v1/results/latest" &&
              r.status() === 200,
          );
          await page.goto("/en/results");
          const response = await reply,
            body: LatestResultResponse = await response.json();
          expect(response.headers()["cache-control"]).toBe("no-store");
          expect(body.result?.match_id).toBe(id);
          expect(body.result?.rules_hash).toBe(hash);
          if (!known)
            expect(body.result).toMatchObject({
              own: null,
              end_elapsed_ms: null,
              result: {
                reason: "server_failure",
                outcome: "abort",
                completed: false,
              },
            });
          else
            expect(body.result).toMatchObject({
              end_elapsed_ms: 243000,
              result: { reason: "timeout", outcome: "draw", completed: true },
              own: { opened_safe: 4 },
            });
          expect(JSON.stringify(body)).not.toMatch(
            /"seed"|"board"|"account"|"provider"|"session"|"opponent"|"csrf"/,
          );
          const result = page.getByTestId("personal-result");
          await expect(result).toBeVisible();
          await expect(page).toHaveURL(`${origin}/en/results`);
          await expect(page.getByRole("grid")).toHaveCount(0);
          expect(
            requested.some(
              (url) =>
                url.includes(id) ||
                new URL(url).pathname === "/api/v1/ws" ||
                new URL(url).pathname === "/api/v1/lobby",
            ),
          ).toBe(false);
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
            await expect(page.getByRole("heading", { level: 1 })).toHaveText(
              messages["result.latest"],
            );
            await expect(result).toContainText(messages["result.savedTitle"]);
            if (!known) {
              await expect(result).toContainText(
                messages["result.statsUnavailable"],
              );
              await expect(result).not.toContainText(/\d/);
            }
          }
          expect(
            await page.evaluate(() =>
              JSON.stringify({
                local: { ...localStorage },
                session: { ...sessionStorage },
              }),
            ),
          ).not.toContain(id);
          expect(
            await page.evaluate(
              () => document.documentElement.scrollWidth <= innerWidth,
            ),
          ).toBe(true);
          await mkdir(".tmp/online-layout", { recursive: true });
          await writeFile(
            `.tmp/online-layout/latest-process-${known ? "known" : "unknown"}-${width}.json`,
            JSON.stringify(process, null, 2),
          );
          await page.screenshot({
            path: `.tmp/online-layout/latest-${known ? "known" : "unknown"}-${width}.png`,
            fullPage: true,
          });
          const before = await (
            await page.request.get(`/__fixture/persisted/${id}`)
          ).json();
          expect(before).toMatchObject({
            active_count: 0,
            result_count: 1,
            retention_started_at: admitted,
          });
          if (!known)
            expect(before).toMatchObject({
              seed_absent: true,
              end_elapsed_ms: null,
              unknown_players: 2,
            });
          const again = await request.post(
            "http://127.0.0.1:3002/__fixture/restart",
          );
          expect(again.status()).toBe(200);
          expect(
            await (await page.request.get(`/__fixture/persisted/${id}`)).json(),
          ).toEqual(before);
          other = await browser.newContext({
            baseURL: origin,
            ignoreHTTPSErrors: true,
          });
          await approve(other);
          const outsider = await other.newPage();
          await login(outsider);
          await outsider.goto("/en/results");
          await expect(
            outsider.getByText("No recent saved result is available."),
          ).toBeVisible();
          await expect(outsider.getByTestId("personal-result")).toHaveCount(0);
          await outsider
            .getByRole("button", { name: "Check stored result" })
            .focus();
          const retry = outsider.waitForResponse(
            (r) =>
              new URL(r.url()).pathname === "/api/v1/results/latest" &&
              r.status() === 200,
          );
          await outsider.keyboard.press("Enter");
          expect(await (await retry).json()).toEqual({ v: 1, result: null });
          await expect(
            outsider.getByText("No recent saved result is available."),
          ).toBeVisible();
          await page.goto("/en/results");
          await expect(page.getByTestId("personal-result")).toBeVisible();
          await fresh.setOffline(true);
          await expect(page.getByTestId("personal-result")).toHaveCount(0);
        } finally {
          await a.close();
          await b.close();
          await fresh?.close();
          await other?.close();
        }
      });
}
