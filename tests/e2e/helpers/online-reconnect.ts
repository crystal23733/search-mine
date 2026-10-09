import { test, expect, type BrowserContext, type Page } from "@playwright/test";
import { stream, advance } from "./online-quick";

type Approve = (context: BrowserContext) => Promise<void>;
type Login = (page: Page) => Promise<void>;
const origin = "https://localhost:8443";

export function registerOnlineReconnectTests(approve: Approve, login: Login) {
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
