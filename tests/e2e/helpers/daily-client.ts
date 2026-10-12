import { test, expect, type BrowserContext, type Page } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { solveDaily } from "./daily-http";
import { advance } from "./online-quick";
import { createAuth } from "../../../apps/web/src/services/auth/session";
import { createAuthHttp } from "../../../apps/web/src/services/auth/http";
import { createDailyHttp } from "../../../apps/web/src/services/daily/http";
import type { DailySubmitResponse } from "../../../packages/protocol/src/index";
type Approve = (context: BrowserContext) => Promise<void>;
type Login = (page: Page) => Promise<void>;

async function observeSubmission(
  page: Page,
  trigger: () => Promise<unknown>,
): Promise<DailySubmitResponse> {
  let delivered!: (body: DailySubmitResponse) => void;
  let failed!: (error: unknown) => void;
  const observed = new Promise<DailySubmitResponse>((resolve, reject) => {
    delivered = resolve;
    failed = reject;
  });
  await page.route(
    "**/api/v1/daily/submit-timed",
    async (route) => {
      try {
        const response = await route.fetch({
          maxRedirects: 0,
          maxRetries: 0,
          timeout: 10000,
        });
        expect(response.status()).toBe(200);
        expect(response.headers()["cache-control"]).toBe("no-store");
        const body: DailySubmitResponse = await response.json();
        // Observe the actual server body before handing that same response to the app.
        await route.fulfill({ response });
        delivered(body);
      } catch (error) {
        failed(error);
        await route.abort();
      }
    },
    { times: 1 },
  );
  const [, body] = await Promise.all([trigger(), observed]);
  return body;
}
export function registerDailyClientTests(approve: Approve, login: Login) {
  test("typed daily client reads start heartbeat completion own and anonymous rankings over real HTTPS", async ({
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
      // This adapter uses the real cookie jar and HTTPS server; stream cancellation is tested separately.
      const fetcher: typeof fetch = async (input, init) => {
        const path = String(input);
        expect(path).toMatch(/^\/api\/v1\/(?:auth|me|daily)\//);
        if (init?.signal?.aborted) throw Error("cancelled");
        const headers = new Headers(init?.headers);
        if (init?.method === "POST")
          headers.set("origin", "https://localhost:8443");
        const reply = await context.request.fetch(path, {
          method: init?.method,
          headers: Object.fromEntries(headers),
          data: init?.body as string | undefined,
          timeout: 10000,
          maxRedirects: 0,
        });
        return new Response(new Uint8Array(await reply.body()).buffer, {
          status: reply.status(),
          headers: reply.headers(),
        });
      };
      const auth = createAuth(createAuthHttp(fetcher));
      try {
        await auth.refresh();
        const owner = { accountId: auth.account()!, revision: auth.revision() };
        const api = createDailyHttp(auth, fetcher);
        const attempt = crypto.randomUUID();
        const start = await api.start(owner, attempt);
        expect(
          (
            await api.definition(
              owner,
              start.metadata.date,
              start.metadata.seed_version,
            )
          ).metadata,
        ).toEqual(start.metadata);
        expect((await api.own(owner, start.metadata)).receipt).toBeNull();
        const replay = await solveDaily(page, start.metadata.date);
        const target = replay.final_time_ms + 10000;
        let elapsed = 0,
          sequence = 0;
        while (elapsed < target) {
          const seconds = Math.min(15, Math.ceil((target - elapsed) / 1000));
          await advance(page, seconds);
          elapsed += seconds * 1000;
          expect(
            (await api.heartbeat(owner, start.ticket_id, ++sequence)).ack,
          ).toEqual({ sequence, elapsed_ms: elapsed });
        }
        const saved = await api.submitTimed(
          owner,
          attempt,
          start.ticket_id,
          replay,
        );
        expect(saved.disposition).toBe("official");
        expect(saved.trust).toBe("verified_time");
        expect(saved.receipt.verified_elapsed_ms).toBe(
          elapsed - start.rules.rules.countdown_ms,
        );
        expect((await api.own(owner, start.metadata)).receipt).toEqual(
          saved.receipt,
        );
        await advance(page, 4);
        expect(
          (await api.submitTimed(owner, crypto.randomUUID(), null, replay))
            .receipt,
        ).toEqual(saved.receipt);
        const anonymous = await browser.newContext({
          baseURL: "https://localhost:8443",
          ignoreHTTPSErrors: true,
        });
        try {
          const publicFetch: typeof fetch = async (path, init) => {
            expect(init?.credentials).toBe("omit");
            const reply = await anonymous.request.post(String(path), {
              headers: {
                "content-type": "application/json",
                origin: "https://localhost:8443",
              },
              data: init?.body as string,
            });
            return new Response(new Uint8Array(await reply.body()).buffer, {
              status: reply.status(),
              headers: reply.headers(),
            });
          };
          const rankings = await createDailyHttp(auth, publicFetch).rankings(
            start.metadata.date,
            1,
          );
          expect(rankings.metadata).toEqual(start.metadata);
          expect(
            rankings.time.entries.some((r) => r.nickname === "é探偵"),
          ).toBe(true);
          expect(
            (await anonymous.cookies()).filter((c) => c.name.includes("liar")),
          ).toHaveLength(0);
        } finally {
          await anonymous.close();
        }
      } finally {
        auth.dispose();
      }
    } finally {
      await context.close();
    }
  });
  for (const width of [1440, 390])
    test(`daily client explicitly submits pending WASM completion in eight locales at ${width}px`, async ({
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
        const posted: string[] = [];
        page.on("request", (request) => {
          if (
            /\/api\/v1\/daily\/(?:definition|submit-timed)$/.test(
              new URL(request.url()).pathname,
            )
          )
            posted.push(request.url());
        });
        let firstCompletion: string | undefined;
        const clock = (await page.request
          .get("/__fixture/clock")
          .then((r) => r.json())) as { utc: number };
        const date = new Date(clock.utc * 1000).toISOString().slice(0, 10);
        const replay = await solveDaily(page, date);
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
          await page.evaluate(async (replay) => {
            const db = await new Promise<IDBDatabase>((resolve, reject) => {
              const request = indexedDB.open("liar.daily.v1", 2);
              request.onupgradeneeded = () => {
                for (const name of ["records", "pending"])
                  request.result.createObjectStore(name, { keyPath: "id" });
              };
              request.onsuccess = () => resolve(request.result);
              request.onerror = () => reject(request.error);
            });
            try {
              await new Promise<void>((resolve, reject) => {
                const tx = db.transaction(["records", "pending"], "readwrite");
                const record = {
                  v: 1,
                  id: `${replay.metadata.date}:v${replay.metadata.seed_version}:${replay.metadata.rules_hash}`,
                  status: "unverified",
                  attempt_id: crypto.randomUUID(),
                  metadata: replay.metadata,
                  elapsed_ms: replay.final_time_ms,
                  mistakes: 0,
                  replay,
                };
                for (const name of ["records", "pending"]) {
                  const store = tx.objectStore(name);
                  store.clear();
                  store.put(record);
                }
                tx.oncomplete = () => resolve();
                tx.onerror = () => reject(tx.error);
              });
            } finally {
              db.close();
            }
          }, replay);
          posted.length = 0;
          await page.goto(`/${locale}/settings`);
          const submit = page.getByRole("button", {
            name: messages["offline.submit"] ?? "Submit pending results",
            exact: true,
          });
          await expect(submit).toBeEnabled();
          const pending = async () =>
            page.evaluate(async () => {
              const db = await new Promise<IDBDatabase>((resolve, reject) => {
                const r = indexedDB.open("liar.daily.v1", 2);
                r.onsuccess = () => resolve(r.result);
                r.onerror = () => reject(r.error);
              });
              try {
                const counts = await Promise.all(
                  ["pending", "records"].map(
                    (name) =>
                      new Promise<number>((resolve, reject) => {
                        const r = db
                          .transaction(name)
                          .objectStore(name)
                          .count();
                        r.onsuccess = () => resolve(r.result);
                        r.onerror = () => reject(r.error);
                      }),
                  ),
                );
                return { pending: counts[0], records: counts[1] };
              } finally {
                db.close();
              }
            });
          expect(await pending()).toEqual({ pending: 1, records: 1 });
          expect(posted).toHaveLength(0);
          await submit.click();
          const dialog = page.getByRole("dialog", {
            name: messages["offline.submit"] ?? "Submit pending results",
          });
          await expect(dialog).toBeVisible();
          expect(await pending()).toEqual({ pending: 1, records: 1 });
          expect(posted).toHaveLength(0);
          if (locale === "en") {
            await page.route("**/api/v1/daily/submit-timed", (route) =>
              route.fulfill({
                status: 503,
                contentType: "application/json",
                body: JSON.stringify({ code: "daily_unavailable" }),
              }),
            );
            await dialog
              .getByRole("button", {
                name: messages["offline.submitConfirm"],
                exact: true,
              })
              .click();
            await expect(
              page
                .getByRole("status")
                .filter({ hasText: messages["offline.submitRetained"] }),
            ).toHaveText(messages["offline.submitRetained"]);
            expect(await pending()).toEqual({ pending: 1, records: 1 });
            await page.unroute("**/api/v1/daily/submit-timed");
            await expect(submit).toBeEnabled();
            await submit.click();
          }
          const saved = await observeSubmission(page, () =>
            dialog
              .getByRole("button", {
                name:
                  messages["offline.submitConfirm"] ?? "Submit to this account",
                exact: true,
              })
              .click(),
          );
          await expect.poll(pending).toEqual({ pending: 0, records: 1 });
          expect(saved.trust).toBe("verified_completion");
          expect(saved.receipt.verified_elapsed_ms).toBeNull();
          expect(saved.disposition).toBe(
            firstCompletion ? "practice" : "official",
          );
          if (!firstCompletion) firstCompletion = saved.receipt.completion_id;
          expect(saved.receipt.completion_id).toBe(firstCompletion);
          await expect(dialog).not.toBeVisible();
          expect(
            await page.evaluate(
              () => document.documentElement.scrollWidth <= innerWidth,
            ),
          ).toBe(true);
        }
      } finally {
        await context.close();
      }
    });
}
