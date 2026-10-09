import { test, expect, type BrowserContext, type Page } from "@playwright/test";
import type {
  OnlineEvent,
  OnlineInput,
} from "../../../packages/protocol/src/index";
import { advance, stream } from "./online-quick";
import { mkdir } from "node:fs/promises";

type Approve = (context: BrowserContext) => Promise<void>;
type Login = (page: Page) => Promise<void>;
const origin = "https://localhost:8443";
async function join(one: Page, two: Page) {
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
}
export function registerOnlineRecoveryTests(approve: Approve, login: Login) {
  for (const width of [1440, 390]) {
    test(`actual lost ACK replays only the epoch and resumes one effect at ${width}px`, async ({
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
      let release!: () => void;
      const gate = new Promise<void>((resolve) => {
        release = resolve;
      });
      try {
        await approve(a);
        await approve(b);
        const one = await a.newPage(),
          two = await b.newPage();
        await login(one);
        await login(two);
        const events: OnlineEvent[] = [],
          commands: OnlineInput[] = [];
        let connections = 0,
          dropped: OnlineEvent | undefined;
        await one.routeWebSocket("**/api/v1/ws", async (client) => {
          const index = ++connections;
          if (index === 2) await gate;
          const server = client.connectToServer();
          client.onMessage((message) => {
            commands.push(JSON.parse(message.toString()));
            server.send(message);
          });
          server.onMessage((message) => {
            const event: OnlineEvent = JSON.parse(message.toString());
            if (index === 1 && event.payload.type === "ack" && !dropped) {
              dropped = event;
              void Promise.all([
                client.close({ code: 1012 }),
                server.close({ code: 1000 }),
              ]);
              return;
            }
            events.push(event);
            client.send(message);
          });
        });
        const peer = stream(two);
        await join(one, two);
        await expect.poll(() => peer.lastView()?.phase).toBe("playing");
        await one.locator('[data-cell="2"]').click({ button: "right" });
        await expect.poll(() => Boolean(dropped)).toBe(true);
        expect(dropped!.payload.type === "ack" && dropped!.payload.status).toBe(
          "applied",
        );
        const original = commands[0];
        const notice = one.getByTestId("own-reconnect");
        await expect(notice).toContainText("Trying to reconnect");
        await expect(one.getByRole("gridcell")).toHaveCount(9);
        await expect(
          one.getByRole("button", { name: "Attack", exact: true }),
        ).toBeDisabled();
        expect((await notice.boundingBox())!.y).toBeLessThan(
          (await one.getByRole("grid").boundingBox())!.y,
        );
        expect(commands).toHaveLength(1);
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
          await one.getByRole("combobox").first().selectOption(locale);
          await expect(one.locator("html")).toHaveAttribute("lang", locale);
          await expect(notice.locator("output")).not.toHaveText("");
          expect(await notice.textContent()).not.toContain(
            "match.reconnecting",
          );
          expect(
            await one.evaluate(
              () => document.documentElement.scrollWidth <= innerWidth,
            ),
          ).toBe(true);
        }
        await one.getByRole("combobox").first().selectOption("en");
        await mkdir(".tmp/online-layout", { recursive: true });
        await one.screenshot({
          path: `.tmp/online-layout/80-recovery-${width}.png`,
          fullPage: true,
        });
        await advance(two, 10);
        release();
        await expect(notice).toHaveCount(0);
        await expect.poll(() => commands.length).toBe(2);
        const replay = commands[1];
        expect(replay).toEqual({
          ...original,
          session_epoch: replay.session_epoch,
        });
        expect(replay.session_epoch).toBeGreaterThan(original.session_epoch);
        const snapshot = events
          .filter((e) => e.payload.type === "snapshot")
          .at(-1)!;
        if (snapshot.payload.type !== "snapshot") throw Error("snapshot");
        expect(snapshot.match_id).toBe(original.match_id);
        expect(snapshot.payload.last_client_seq).toBe(original.client_seq);
        expect(snapshot.payload.view.own.cells[2].flagged).toBe(true);
        expect(snapshot.payload.view.remaining_ms).toBeLessThan(240000);
        await expect
          .poll(() =>
            events.some(
              (e) =>
                e.payload.type === "ack" &&
                e.payload.command_id === original.command_id &&
                e.payload.duplicate &&
                e.payload.revision ===
                  (dropped!.payload.type === "ack"
                    ? dropped!.payload.revision
                    : -1),
            ),
          )
          .toBe(true);
        await expect(one.locator('[data-cell="2"]')).toHaveAttribute(
          "aria-label",
          /flag/i,
        );
        await one.locator('[data-cell="5"]').click({ button: "right" });
        await expect(one.locator('[data-cell="5"]')).toHaveAttribute(
          "aria-label",
          /flag/i,
        );
        expect(commands.at(-1)!.client_seq).toBe(original.client_seq + 1);
        expect(peer.lastView()?.own.cells[2].flagged).toBe(false);
        const stored = await one.evaluate(() =>
          JSON.stringify({ ...localStorage, ...sessionStorage }),
        );
        for (const secret of [original.match_id, original.command_id])
          expect(stored).not.toContain(secret);
        expect(stored).not.toMatch(/seed|credential|online_input/);
        for (const e of events)
          expect(JSON.stringify(e)).not.toMatch(/"seed"|"truth"|"account_id"/);
        await advance(two, 240);
        await expect(one.getByTestId("recording")).toHaveText("Result saved.");
        await expect(two.getByTestId("recording")).toHaveText("Result saved.");
        await expect(one.getByRole("alert")).toHaveCount(0);
      } finally {
        release();
        await a.close();
        await b.close();
      }
    });
    test(`actual offline board stays read-only, recovers and rejects a revoked session at ${width}px`, async ({
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
      let revoker: BrowserContext | undefined;
      try {
        await approve(a);
        await approve(b);
        const one = await a.newPage(),
          two = await b.newPage();
        await login(one);
        await login(two);
        const own = stream(one),
          peer = stream(two);
        await join(one, two);
        await expect.poll(() => own.lastView()?.phase).toBe("playing");
        await one.locator('[data-cell="2"]').click({ button: "right" });
        await expect(one.locator('[data-cell="2"]')).toHaveAttribute(
          "aria-label",
          /flag/i,
        );
        const commands = own.commands.length;
        await a.setOffline(true);
        await expect(one.getByTestId("own-reconnect")).toBeVisible();
        await expect(one.getByRole("gridcell")).toHaveCount(9);
        await expect(
          one.getByRole("button", { name: "Attack", exact: true }),
        ).toBeDisabled();
        await expect
          .poll(() => peer.lastView()?.opponent.reconnect_ms)
          .toBe(30000);
        await advance(two, 10);
        expect(own.commands).toHaveLength(commands);
        await a.setOffline(false);
        await expect(one.getByTestId("own-reconnect")).toHaveCount(0);
        await expect
          .poll(
            () =>
              own.events.filter((e) => e.payload.type === "snapshot").length,
          )
          .toBe(2);
        await expect(one.locator('[data-cell="2"]')).toHaveAttribute(
          "aria-label",
          /flag/i,
        );
        expect(own.lastView()?.remaining_ms).toBeLessThan(240000);
        const snapshots = own.events.filter(
          (e) => e.payload.type === "snapshot",
        );
        expect(snapshots[1].match_id).toBe(snapshots[0].match_id);
        revoker = await browser.newContext({
          baseURL: origin,
          ignoreHTTPSErrors: true,
          storageState: await a.storageState(),
        });
        await a.setOffline(true);
        await expect(one.getByTestId("own-reconnect")).toBeVisible();
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
        await a.setOffline(false);
        await expect(one.getByRole("gridcell")).toHaveCount(0);
        await expect(one.getByTestId("own-reconnect")).toHaveCount(0);
        await expect(
          one.getByRole("link", { name: "Practice without signing in" }),
        ).toBeVisible();
        expect(
          own.events.filter((e) => e.payload.type === "snapshot"),
        ).toHaveLength(2);
        expect(own.commands).toHaveLength(commands);
      } finally {
        await revoker?.close();
        await a.close();
        await b.close();
      }
    });
  }
}
