import { test, expect, type BrowserContext, type Page } from "@playwright/test";
import type {
  OnlineEvent,
  PersonalResultResponse,
} from "../../../packages/protocol/src/index";
import { advance, stream } from "./online-quick";
import { mkdir, readFile } from "node:fs/promises";
type Approve = (context: BrowserContext) => Promise<void>;
type Login = (page: Page) => Promise<void>;
const origin = "https://localhost:8443";
async function loseSavedNotice(one: Page, two: Page) {
  const dropped: OnlineEvent[] = [];
  await one.routeWebSocket("**/api/v1/ws", (client) => {
    const server = client.connectToServer();
    client.onMessage((message) => server.send(message));
    server.onMessage((message) => {
      const event: OnlineEvent = JSON.parse(message.toString());
      if (
        event.payload.type === "match_end" &&
        event.payload.recording === "saved"
      )
        dropped.push(event);
      else client.send(message);
    });
    // The real terminal close is forwarded; only the saved frame is lost.
  });
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
  await advance(two, 3);
  await expect.poll(() => peer.lastView()?.phase).toBe("playing");
  await advance(two, 240);
  await expect(two.getByTestId("recording")).toHaveText("Result saved.");
  await expect(one.getByTestId("recording")).toHaveText(
    "Confirming the result…",
  );
  await expect.poll(() => dropped.length).toBeGreaterThan(0);
  return { own, peer, matchId: dropped[0].match_id };
}
async function storedRequest(context: BrowserContext, matchId: string) {
  const proof = await (
    await context.request.get("/api/v1/auth/bootstrap")
  ).json();
  return context.request.post("/api/v1/results", {
    data: { v: 1, match_id: matchId },
    headers: { Origin: origin, "x-liar-csrf": proof.csrf },
  });
}
export function registerPersonalResultTests(approve: Approve, login: Login) {
  for (const width of [1440, 390]) {
    for (const unknown of [false, true]) {
      test(`actual ${unknown ? "unknown abort contract" : "saved personal result"} survives lost notice and actor cleanup at ${width}px`, async ({
        browser,
      }) => {
        const a = await browser.newContext({
          baseURL: origin,
          ignoreHTTPSErrors: true,
          viewport: { width, height: 900 },
        });
        const b = await browser.newContext({
          baseURL: origin,
          ignoreHTTPSErrors: true,
        });
        const other = await browser.newContext({
          baseURL: origin,
          ignoreHTTPSErrors: true,
        });
        try {
          await approve(a);
          await approve(b);
          await approve(other);
          const one = await a.newPage(),
            two = await b.newPage();
          await login(one);
          await login(two);
          const { matchId, peer } = await loseSavedNotice(one, two);
          await advance(two, 31);
          await expect
            .poll(async () =>
              (await one.request.get(`/__fixture/match/${matchId}`)).status(),
            )
            .toBe(404);
          if (unknown) {
            // Seed only the contract; actual journal/startup recovery is a later issue.
            expect(
              (
                await one.request.post(`/__fixture/result/${matchId}/unknown`)
              ).status(),
            ).toBe(200);
          }
          const requests: string[] = [];
          one.on("request", (r) => requests.push(r.url()));
          let replies = 0;
          await one.route("**/api/v1/results", (route) => {
            replies++;
            return route.fulfill({
              status: replies === 1 ? 503 : 404,
              json: { error: replies === 1 ? "unavailable" : "not_found" },
            });
          });
          await one
            .getByRole("button", { name: "Check stored result" })
            .click();
          await expect(
            one.getByText("The result could not be checked. Try again."),
          ).toBeVisible();
          await expect(one.getByTestId("personal-result")).toHaveCount(0);
          await one
            .getByRole("button", { name: "Check stored result" })
            .click();
          await expect(
            one.getByText("No stored result is available for this match."),
          ).toBeVisible();
          await expect(one.getByTestId("personal-result")).toHaveCount(0);
          await one.unroute("**/api/v1/results");
          const read = one.waitForResponse(
            (r) => r.url().endsWith("/api/v1/results") && r.status() === 200,
          );
          await one
            .getByRole("button", { name: "Check stored result" })
            .focus();
          await one.keyboard.press("Enter");
          const response = await read;
          expect(response.headers()["cache-control"]).toBe("no-store");
          const body: PersonalResultResponse = await response.json();
          expect(Object.keys(body.result).sort()).toEqual(
            [
              "match_id",
              "rules_hash",
              "end_elapsed_ms",
              "result",
              "own",
            ].sort(),
          );
          if (unknown) {
            expect(body.result.own).toBeNull();
            expect(body.result.end_elapsed_ms).toBeNull();
            expect(body.result.result).toEqual({
              reason: "server_failure",
              outcome: "abort",
              completed: false,
            });
          } else {
            expect(body.result.own).not.toBeNull();
            expect(Object.keys(body.result.own!).sort()).toEqual(
              [
                "opened_safe",
                "mistakes",
                "accusation_attempts",
                "correct_accusations",
              ].sort(),
            );
          }
          expect(body.result.match_id).toBe(matchId);
          expect(body.result.rules_hash).toBe(peer.lastView()!.rules.hash);
          expect(body.result.result.reason).toBe(
            unknown ? "server_failure" : "timeout",
          );
          expect(JSON.stringify(body)).not.toMatch(
            /"seed"|"board"|"account"|"provider"|"session"|"opponent"|"csrf"/,
          );
          const result = one.getByTestId("personal-result");
          await expect(result).toBeVisible();
          await expect(result.getByTestId("recording")).toHaveText(
            "Result saved.",
          );
          await expect(one.getByRole("grid")).toHaveCount(0);
          await expect(one.getByRole("progressbar")).toHaveCount(0);
          await expect(one.locator("details")).toHaveCount(0);
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
            await one.getByRole("combobox").first().selectOption(locale);
            await expect(one.locator("html")).toHaveAttribute("lang", locale);
            await expect(result).toContainText(messages["result.savedTitle"]);
            if (unknown) {
              await expect(result).toContainText(
                messages["result.statsUnavailable"],
              );
              await expect(result).toContainText(
                messages["match.result.abort"],
              );
              await expect(result).toContainText(
                messages["match.reason.server_failure"],
              );
              await expect(result).not.toContainText(/\d/);
            }
            await expect(result.getByRole("button")).toHaveText(
              messages["queue.again"],
            );
            expect(
              await one.evaluate(
                () => document.documentElement.scrollWidth <= innerWidth,
              ),
            ).toBe(true);
          }
          expect(
            requests.filter((url) => url.endsWith("/api/v1/results")),
          ).toHaveLength(3);
          expect(
            requests.filter((url) =>
              /doubleclick|googlesyndication|adsbygoogle/.test(url),
            ),
          ).toHaveLength(0);
          const storage = await one.evaluate(() =>
            JSON.stringify({ ...localStorage, ...sessionStorage }),
          );
          expect(storage).not.toContain(matchId);
          expect(storage).not.toMatch(
            /csrf|credential|rules_hash|online_input/,
          );
          await mkdir(".tmp/online-layout", { recursive: true });
          await one.screenshot({
            path: `.tmp/online-layout/${unknown ? "93-unknown" : "89-personal-result"}-${width}.png`,
            fullPage: true,
          });
          const outsider = await other.newPage();
          await login(outsider);
          const forbidden = await storedRequest(other, matchId);
          const absent = await storedRequest(
            other,
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
          );
          expect(forbidden.status()).toBe(404);
          expect(await forbidden.json()).toEqual(await absent.json());
          await result.getByRole("button").click();
          await expect(one.getByTestId("personal-result")).toHaveCount(0);
        } finally {
          await a.close();
          await b.close();
          await other.close();
        }
      });
    }
  }
  test("actual revoked session cannot publish a previously captured HTTP result", async ({
    browser,
  }) => {
    const a = await browser.newContext({
      baseURL: origin,
      ignoreHTTPSErrors: true,
    });
    const b = await browser.newContext({
      baseURL: origin,
      ignoreHTTPSErrors: true,
    });
    let release!: () => void;
    const gate = new Promise<void>((r) => {
      release = r;
    });
    let revoker: BrowserContext | undefined;
    try {
      await approve(a);
      await approve(b);
      const one = await a.newPage(),
        two = await b.newPage();
      await login(one);
      await login(two);
      await loseSavedNotice(one, two);
      let captured = false;
      await one.route("**/api/v1/results", async (route) => {
        const response = await route.fetch();
        expect(response.status()).toBe(200);
        captured = true;
        await gate;
        await route.fulfill({ response });
      });
      await one.getByRole("button", { name: "Check stored result" }).click();
      await expect.poll(() => captured).toBe(true);
      await expect(
        one.getByRole("button", { name: "Check stored result" }),
      ).toBeDisabled();
      revoker = await browser.newContext({
        baseURL: origin,
        ignoreHTTPSErrors: true,
        storageState: await a.storageState(),
      });
      const proof = await (
        await revoker.request.get("/api/v1/auth/bootstrap")
      ).json();
      expect(
        (
          await revoker.request.post("/api/v1/auth/logout", {
            data: {},
            headers: { Origin: origin, "x-liar-csrf": proof.csrf },
          })
        ).status(),
      ).toBe(204);
      release();
      await expect(
        one.getByRole("link", { name: "Practice without signing in" }),
      ).toBeVisible();
      await expect(one.getByTestId("personal-result")).toHaveCount(0);
      await expect(one.getByTestId("recording")).toHaveCount(0);
      await expect(one.getByRole("grid")).toHaveCount(0);
    } finally {
      release();
      await revoker?.close();
      await a.close();
      await b.close();
    }
  });
}
