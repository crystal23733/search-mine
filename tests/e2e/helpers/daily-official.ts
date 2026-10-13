import { test, expect, type BrowserContext, type Page } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { solveDaily } from "./daily-http";
import { advance } from "./online-quick";
import type {
  DailyStartResponse,
  DailySubmitResponse,
} from "../../../packages/protocol/src/index";
type Approve = (context: BrowserContext) => Promise<void>;
type Login = (page: Page) => Promise<void>;
async function observe<T>(
  page: Page,
  endpoint: string,
  trigger: () => Promise<unknown>,
  failFirst = false,
): Promise<T> {
  let resolve!: (value: T) => void, reject!: (reason: unknown) => void;
  const observed = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  let failedOnce = false;
  await page.route(
    `**/api/v1/daily/${endpoint}`,
    async (route) => {
      if (failFirst && !failedOnce) {
        failedOnce = true;
        await route.fulfill({
          status: 503,
          contentType: "application/json",
          body: JSON.stringify({ code: "daily_unavailable" }),
        });
        return;
      }
      try {
        const response = await route.fetch({
          maxRedirects: 0,
          maxRetries: 0,
          timeout: 10000,
        });
        expect(response.status()).toBe(200);
        expect(response.headers()["cache-control"]).toBe("no-store");
        const body: T = await response.json();
        await route.fulfill({ response });
        resolve(body);
      } catch (error) {
        reject(error);
        await route.abort();
      }
    },
    { times: failFirst ? 2 : 1 },
  );
  const [, body] = await Promise.all([trigger(), observed]);
  return body;
}
async function stored(page: Page) {
  return page.evaluate(async () => {
    const db = await new Promise<IDBDatabase>((resolve, reject) => {
      const r = indexedDB.open("liar.daily.v1", 2);
      r.onsuccess = () => resolve(r.result);
      r.onerror = () => reject(r.error);
    });
    try {
      return await Promise.all(
        ["records", "pending"].map(
          (name) =>
            new Promise<number>((resolve, reject) => {
              const r = db.transaction(name).objectStore(name).count();
              r.onsuccess = () => resolve(r.result);
              r.onerror = () => reject(r.error);
            }),
        ),
      );
    } finally {
      db.close();
    }
  });
}
export function registerDailyOfficialTests(approve: Approve, login: Login) {
  for (const width of [1440, 390])
    test(`official daily explicit challenge and first server receipt at ${width}px`, async ({
      browser,
    }) => {
      test.setTimeout(120000); // Two full boards use real DOM input, HTTPS and PostgreSQL.
      const context = await browser.newContext({
        baseURL: "https://localhost:8443",
        ignoreHTTPSErrors: true,
        viewport: { width, height: 900 },
      });
      try {
        await approve(context);
        const page = await context.newPage();
        await login(page);
        await page.goto("/en/daily");
        const official = page.getByRole("link", {
          name: "Official daily challenge",
          exact: true,
        });
        await expect(official).toBeVisible();
        await official.click();
        await expect(
          page.getByRole("heading", {
            level: 1,
            name: "Official daily challenge",
            exact: true,
          }),
        ).toBeVisible();
        const clock: { utc: number } = await page.request
          .get("/__fixture/clock")
          .then((r) => r.json());
        const date = new Date(clock.utc * 1000).toISOString().slice(0, 10);
        const replay = await solveDaily(page, date);
        await page.clock.install({ time: new Date(clock.utc * 1000) });
        await page.clock.pauseAt(new Date(clock.utc * 1000 + 1000));
        const starts: string[] = [],
          submissions: string[] = [];
        let beats = 0;
        page.on("request", (request) => {
          if (request.url().endsWith("/api/v1/daily/start"))
            starts.push(request.postDataJSON().attempt_id);
          if (request.url().endsWith("/api/v1/daily/submit-timed"))
            submissions.push(request.postDataJSON().attempt_id);
        });
        await page.route("**/api/v1/daily/heartbeat", async (route) => {
          const response = await route.fetch({
            maxRedirects: 0,
            maxRetries: 0,
            timeout: 10000,
          });
          expect(response.status()).toBe(200);
          expect((await response.json()).ack).not.toBeNull();
          await route.fulfill({ response });
          beats++;
        });
        let first: DailySubmitResponse | undefined;
        for (const practice of [false, true]) {
          await advance(page, 4);
          const before = starts.length;
          await page
            .getByRole("button", {
              name: "Start official challenge",
              exact: true,
            })
            .click();
          await page.clock.runFor(200);
          const dialog = page.getByRole("dialog");
          await expect(dialog).toContainText("é探偵");
          expect(starts).toHaveLength(before);
          const grant = await observe<DailyStartResponse>(page, "start", () =>
            dialog.getByRole("button", { name: "Confirm and start" }).click(),
          );
          expect(grant.metadata).toEqual(replay.metadata);
          await expect(page.getByRole("grid")).toBeVisible();
          const saved = await observe<DailySubmitResponse>(
            page,
            "submit-timed",
            async () => {
              let local = 0,
                server = 0,
                previous = 0;
              const elapse = async (ms: number) => {
                const next = local + ms,
                  target = Math.ceil(next / 1000);
                if (target > server) {
                  await advance(page, target - server);
                  server = target;
                }
                await page.clock.fastForward(ms);
                local = next;
              };
              for (const entry of replay.inputs) {
                expect(entry.input.action.type).toBe("open");
                await elapse(Math.max(250, entry.time_ms - previous) + 250);
                previous = entry.time_ms;
                const cell = page.locator(
                  `[data-cell="${"cell" in entry.input.action ? entry.input.action.cell : -1}"]`,
                );
                // Worker replies arrive asynchronously; drive the paused clock until the public stun clears.
                await expect
                  .poll(async () => {
                    if ((await cell.getAttribute("aria-disabled")) === "true")
                      await elapse(250);
                    return cell.getAttribute("aria-disabled");
                  })
                  .toBe("false");
                await cell.press("Enter");
                await expect(cell).not.toHaveAttribute("data-state", "closed");
              }
              if (practice) {
                await expect(
                  page.getByText(
                    "Submission failed. Your local candidate is kept; retry explicitly or submit from Settings.",
                    { exact: true },
                  ),
                ).toBeVisible();
                expect(await stored(page)).toEqual([1, 0]); // The earlier first local record remains intact.
                expect(submissions.at(-1)).toBe(starts.at(-1));
                await page
                  .getByRole("button", { name: "Retry verification" })
                  .click();
              }
            },
            practice,
          );
          expect(saved.disposition).toBe(practice ? "practice" : "official");
          expect(saved.trust).toBe("verified_time");
          expect(saved.receipt.verified_elapsed_ms).not.toBeNull();
          if (first) expect(saved.receipt).toEqual(first.receipt);
          else first = saved;
          await expect(
            page.getByRole("heading", { name: "Your verified daily record" }),
          ).toBeVisible();
          await expect(
            page.getByText(
              practice
                ? "Practice completed. Your original first record is shown."
                : "Your first verified completion is shown.",
              { exact: true },
            ),
          ).toBeVisible();
          await expect.poll(() => stored(page)).toEqual([1, 0]);
        }
        expect(beats).toBeGreaterThan(0);
        expect(new Set(starts).size).toBe(2);
        expect(submissions).toHaveLength(3);
        expect(submissions[2]).toBe(submissions[1]);
        const download = page.waitForEvent("download");
        await page.getByRole("button", { name: "Download PNG" }).click();
        const png = await download;
        expect(
          (await readFile((await png.path())!)).subarray(0, 8).toString("hex"),
        ).toBe("89504e470d0a1a0a");
        await page.screenshot({
          path: `.tmp/daily132-result-${width}.png`,
          fullPage: true,
        });
        await page.clock.resume();
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
          const messages: Record<string, string> = JSON.parse(
            await readFile(
              new URL(
                `../../../apps/web/src/locales/${locale}/common.json`,
                import.meta.url,
              ),
              "utf8",
            ),
          );
          await page.goto(`/${locale}/daily/official`);
          await expect(
            page.getByRole("heading", {
              level: 1,
              name: messages["official.title"],
            }),
          ).toBeVisible();
          await page
            .getByRole("button", { name: messages["official.own"] })
            .click();
          await expect(
            page.getByRole("heading", { name: messages["official.result"] }),
          ).toBeVisible();
          await expect(
            page.getByText(messages["official.verifiedTime"], { exact: true }),
          ).toBeVisible();
          expect(
            await page.evaluate(
              () => document.documentElement.scrollWidth <= innerWidth + 1,
            ),
          ).toBe(true);
        }
        expect(starts).toHaveLength(2);
        expect(submissions).toHaveLength(3);
        await context.setOffline(true);
        await expect(
          page.getByRole("heading", {
            name: "Votre résultat quotidien vérifié",
          }),
        ).toHaveCount(0);
      } finally {
        await context.close();
      }
    });
}
