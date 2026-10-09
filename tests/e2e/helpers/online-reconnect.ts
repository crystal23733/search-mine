import { test, expect, type BrowserContext, type Page } from "@playwright/test";
import { stream, advance } from "./online-quick";
import { mkdir } from "node:fs/promises";

type Approve = (context: BrowserContext) => Promise<void>;
type Login = (page: Page) => Promise<void>;
const origin = "https://localhost:8443";

export function registerOnlineReconnectTests(approve: Approve, login: Login) {
  for (const width of [1440, 390])
    test(`actual peer grace follows disconnect, resume and SQL forfeit at ${width}px`, async ({
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
      try {
        await approve(a);
        await approve(b);
        const one = await a.newPage(),
          two = await b.newPage();
        await login(one);
        await login(two);
        const frames = stream(one),
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
        await expect.poll(() => frames.lastView()?.phase).toBe("playing");
        const original = peer.snapshot()!;
        await two.goto("/en/");
        await expect
          .poll(() => frames.lastView()?.opponent.reconnect_ms)
          .toBe(30000);
        const notice = one.getByTestId("opponent-reconnect");
        await expect(notice).toContainText("Opponent is reconnecting");
        expect((await notice.boundingBox())!.y).toBeLessThan(
          (await one.getByRole("grid").boundingBox())!.y,
        );
        await expect(one.getByTestId("recording")).toHaveCount(0);
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
          await one
            .getByRole("combobox", {
              name: /Language|언어|言語|语言|Idioma|Sprache|Langue/,
            })
            .first()
            .selectOption(locale);
          await expect(one.locator("html")).toHaveAttribute("lang", locale);
          await expect(notice.locator("output")).not.toHaveText("");
          expect(await notice.textContent()).not.toContain(
            "match.opponentReconnecting",
          );
          expect(
            await one.evaluate(
              () => document.documentElement.scrollWidth <= innerWidth,
            ),
          ).toBe(true);
        }
        await one.getByRole("combobox").first().selectOption("en");
        await expect(notice).toContainText("Opponent is reconnecting");
        await mkdir(".tmp/online-layout", { recursive: true });
        await one.screenshot({
          path: `.tmp/online-layout/76-grace-${width}.png`,
          fullPage: true,
        });
        await one.locator('[data-cell="2"]').click({ button: "right" });
        await expect(one.locator('[data-cell="2"]')).toHaveAttribute(
          "aria-label",
          /flag/i,
        );
        expect(frames.lastView()?.result).toBeNull();
        await advance(one, 10);
        await two.goto("/en/queue");
        await expect(two.getByRole("gridcell")).toHaveCount(9);
        await expect
          .poll(() => frames.lastView()?.opponent.reconnect_ms)
          .toBeNull();
        await expect(notice).toHaveCount(0);
        const resumed = peer.events
          .filter((e) => e.payload.type === "snapshot")
          .at(-1)!;
        expect(resumed.match_id).toBe(original.match_id);
        if (
          original.payload.type !== "snapshot" ||
          resumed.payload.type !== "snapshot"
        )
          throw Error("snapshot");
        expect(resumed.payload.session_epoch).toBeGreaterThan(
          original.payload.session_epoch,
        );
        expect(resumed.payload.view.remaining_ms).toBeLessThan(240000);
        expect(
          frames.events.filter((e) => e.payload.type === "snapshot"),
        ).toHaveLength(1);
        await two.goto("/en/");
        await expect
          .poll(() => frames.lastView()?.opponent.reconnect_ms)
          .toBe(30000);
        await advance(one, 30);
        await expect(one.getByTestId("recording")).toHaveText("Result saved.");
        expect(frames.lastView()?.result).toEqual({
          reason: "forfeit",
          outcome: "win",
          completed: true,
        });
        await expect(notice).toHaveCount(0);
        for (const event of frames.events)
          expect(JSON.stringify(event)).not.toMatch(
            /"seed"|"truth"|"account_id"|"opponent_last_client_seq"/,
          );
      } finally {
        await a.close();
        await b.close();
      }
    });
  for (const width of [1440, 390])
    test(`actual reconnect reload restores the consumed cursor and applies the next input at ${width}px`, async ({
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
      try {
        await approve(a);
        await approve(b);
        const one = await a.newPage(),
          two = await b.newPage();
        await login(one);
        await login(two);
        const frames = stream(one);
        const peer = stream(two);
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
        await expect.poll(() => frames.lastView()?.phase).toBe("playing");
        const cell = one.locator('[data-cell="2"]');
        await cell.click({ button: "right" });
        await expect(cell).toHaveAttribute("aria-label", /flag/i);
        const first = frames.commands.at(-1)!;
        expect(first.client_seq).toBe(1);
        await expect
          .poll(() =>
            frames.events.some(
              (e) =>
                e.payload.type === "ack" &&
                e.payload.command_id === first.command_id,
            ),
          )
          .toBe(true);
        await advance(one, 7);
        await one.reload();
        await expect(one.getByRole("gridcell")).toHaveCount(9);
        await expect
          .poll(
            () =>
              frames.events.filter((e) => e.payload.type === "snapshot").length,
          )
          .toBe(2);
        const snapshot = frames.events
          .filter((e) => e.payload.type === "snapshot")
          .at(-1)!;
        expect(snapshot.match_id).toBe(first.match_id);
        if (snapshot.payload.type !== "snapshot") throw Error("snapshot");
        expect(snapshot.payload.last_client_seq).toBe(1);
        expect(snapshot.payload.session_epoch).toBeGreaterThan(
          first.session_epoch,
        );
        expect(snapshot.payload.view.remaining_ms).toBeLessThan(240000);
        expect(snapshot.payload.view.own.cells[2].flagged).toBe(true);
        await cell.click({ button: "right" });
        await expect(cell).not.toHaveAttribute("aria-label", /flag/i);
        const next = frames.commands.at(-1)!;
        expect(next.client_seq).toBe(2);
        expect(next.session_epoch).toBe(snapshot.payload.session_epoch);
        expect(next.command_id).not.toBe(first.command_id);
        await expect
          .poll(() =>
            frames.events.some(
              (e) =>
                e.payload.type === "ack" &&
                e.payload.command_id === next.command_id &&
                e.payload.status === "applied",
            ),
          )
          .toBe(true);
        expect(frames.lastView()?.own.gauge).toBe(0);
        expect(
          peer.events.filter((e) => e.payload.type === "snapshot"),
        ).toHaveLength(1);
        expect(peer.lastView()?.own.cells[2].flagged).toBe(false);
        expect(
          await one.evaluate(() =>
            JSON.stringify({ ...localStorage, ...sessionStorage }),
          ),
        ).not.toContain(first.match_id);
        for (const event of frames.events)
          expect(JSON.stringify(event)).not.toMatch(
            /"seed"|"truth"|"account_id"|"opponent_last_client_seq"/,
          );
        await advance(one, 240);
        await expect(one.getByTestId("recording")).toHaveText("Result saved.");
        await expect(two.getByTestId("recording")).toHaveText("Result saved.");
      } finally {
        await a.close();
        await b.close();
      }
    });
}
