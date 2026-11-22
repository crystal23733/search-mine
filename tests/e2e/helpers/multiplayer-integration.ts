import {
  expect,
  test,
  type BrowserContext,
  type Locator,
  type Page,
} from "@playwright/test";
import type { RoundEvent } from "../../../packages/protocol/src/index";
import { advance } from "./online-quick";

const names = [
  "North Compass",
  "East Compass",
  "South Compass",
  "West Compass",
];
const button = (scope: Page | Locator, name: string) =>
  scope.getByRole("button", { name, exact: true });
const cell = (page: Page, index: number) =>
  page.locator(`[data-cell="${index}"]`);
const publicRoom = (page: Page, title: string) =>
  page
    .getByRole("region", { name: "Public rooms" })
    .getByRole("listitem")
    .filter({ hasText: title });

function observeRound(page: Page) {
  const views: Extract<RoundEvent["payload"], { view: unknown }>["view"][] = [];
  const frames: RoundEvent[] = [];
  page.on("websocket", (socket) => {
    expect(new URL(socket.url()).pathname).toBe("/api/v2/ws");
    socket.on("framereceived", ({ payload }) => {
      if (typeof payload !== "string") return;
      const frame: RoundEvent = JSON.parse(payload);
      frames.push(frame);
      if ("view" in frame.payload) views.push(frame.payload.view);
    });
  });
  return {
    frames,
    view: () => views.at(-1),
    round: () => frames.at(-1)?.round_id,
  };
}

async function readyAndStart(pages: Page[]) {
  const host = pages[0];
  const start = button(host, "Start round");
  await expect(start).toBeDisabled();
  for (const [index, page] of pages.entries()) {
    const ready = button(page, "Ready");
    if (index > 0) {
      const previous = page
        .locator(".rooms-lobby li")
        .filter({ hasText: names[index - 1] });
      await expect(previous.getByText("Ready", { exact: true })).toBeVisible();
    }
    await ready.click();
  }
  await expect(start).toBeEnabled();
  const last = pages.at(-1)!;
  await button(last, "Not ready").click();
  await expect(start).toBeDisabled();
  await button(last, "Ready").click();
  await start.click();
  await Promise.all(
    pages.map((page) => expect(page.getByRole("gridcell")).toHaveCount(9)),
  );
  await advance(host, 3);
  await Promise.all(
    pages.map((page) =>
      expect(page.getByText(/^Time remaining /)).toBeVisible(),
    ),
  );
}

export function registerMultiplayerIntegrationTest(
  approve: (context: BrowserContext) => Promise<void>,
  login: (page: Page, nickname: string) => Promise<void>,
) {
  test("four-player multiplayer integration covers public discovery, random attack, rank and rematch", async ({
    browser,
    page: visitor,
  }) => {
    test.setTimeout(65_000);
    const title = `Compass Race ${crypto.randomUUID().slice(0, 8)}`;
    const contexts: BrowserContext[] = [];
    try {
      for (const width of [1440, 390, 360, 1024])
        contexts.push(
          await browser.newContext({
            baseURL: "https://localhost:8443",
            ignoreHTTPSErrors: true,
            viewport: { width, height: 844 },
          }),
        );
      const pages: Page[] = [];
      for (const [index, context] of contexts.entries()) {
        await approve(context);
        const page = await context.newPage();
        await login(page, names[index]);
        pages.push(page);
      }
      const [host, east, south, west] = pages;
      const wires = pages.map(observeRound);
      await host.goto("/en/rooms");
      await host.getByLabel("Room name").fill(title);
      await button(host, "Create room").click();
      await expect(host.getByRole("heading", { name: title })).toBeVisible();
      const code = await host
        .getByTestId("multiplayer-room-code")
        .textContent();
      async function join(page: Page) {
        await page.goto("/en/rooms");
        await button(publicRoom(page, title), "Join room").click();
        await expect(page.getByRole("heading", { name: title })).toBeVisible();
      }
      await join(east);
      await join(south);
      await expect(host.getByText(names[2], { exact: true })).toBeVisible();
      await readyAndStart([host, east, south]);
      const firstRound = wires[0].round();
      expect(firstRound).toBeTruthy();

      await join(west);
      await expect(
        west.getByText("Waiting for the next round", { exact: true }),
      ).toBeVisible();
      await expect(west.getByRole("gridcell")).toHaveCount(0);
      await visitor.goto("/en/rooms");
      await expect(
        button(publicRoom(visitor, title), "Join room"),
      ).toBeDisabled();
      await expect(publicRoom(visitor, title)).toContainText("4/4");
      for (const index of [8, 2, 6]) await cell(host, index).click();
      await expect(east.getByText("Tied for 2", { exact: true })).toHaveCount(
        2,
      );
      await expect(
        west.getByRole("heading", { name: "Results", exact: true }),
      ).toHaveCount(0);

      await expect(west.getByText(/^Results ·/)).toBeVisible();
      await readyAndStart(pages);
      expect(wires[0].round()).not.toBe(firstRound);
      const newRound = wires[0].round();
      const initialCells = wires[0].view()!.own.cells;
      for (const [index, wire] of wires.entries()) {
        expect(wire.round()).toBe(newRound);
        expect(wire.view()?.opponents).toHaveLength(3);
        expect(wire.view()?.own.cells).toEqual(initialCells);
        const snapshot = wire.frames.find(
          (frame) =>
            frame.round_id === newRound && frame.payload.type === "snapshot",
        );
        expect(snapshot?.payload).toMatchObject({
          own_seat: index,
          last_client_seq: 0,
        });
        const text = JSON.stringify(
          wire.frames.filter((frame) => frame.round_id === newRound),
        );
        expect(text).not.toMatch(
          /"(account_id|seed|secret_seed|invite_code|source_seat|target_seat)"/,
        );
        expect(text).not.toContain(code);
        for (const frame of wire.frames.filter(
          (frame) => "view" in frame.payload,
        ))
          expect(JSON.stringify(frame.payload).match(/"cells"/g)).toHaveLength(
            1,
          );
      }

      for (const page of pages) await cell(page, 8).click();
      for (const wire of wires)
        await expect.poll(() => wire.view()?.own.cells[8].state).toBe("safe");
      const attack = button(host, "Attack");
      await expect(attack).toBeEnabled();
      await attack.click();
      await expect.poll(() => wires[0].view()?.own.gauge).toBe(0);
      for (const page of [east, south, west]) await cell(page, 2).click();
      for (const wire of wires.slice(1))
        await expect.poll(() => wire.view()?.own.cells[2].state).toBe("safe");
      const targets = wires.slice(1).flatMap((wire, index) => {
        const square = wire.view()?.own.cells[2];
        return square?.state === "safe" && square.number === 2
          ? [index + 1]
          : [];
      });
      expect(targets).toHaveLength(1);
      const [target] = targets;
      await button(pages[target], "Accuse").click();
      await cell(pages[target], 2).click();
      await expect
        .poll(() => wires[target].view()?.own.cells[2])
        .toMatchObject({ state: "safe", number: 1 });
      await expect.poll(() => wires[0].view()?.own.stun_ms).toBe(2000);
      for (const wire of wires.slice(1))
        expect(wire.view()?.own.stun_ms).toBe(0);
      await button(pages[target], "Open").click();

      await button(host, "Leave room").click();
      await expect(
        east.locator(".rooms-lobby li").filter({ hasText: names[1] }),
      ).toContainText("Host");
      await cell(east, 6).click();
      for (const page of [east, south, west]) {
        await expect(page.getByText("Rank 1", { exact: true })).toBeVisible();
        await expect(page.getByText("Tied for 2", { exact: true })).toHaveCount(
          2,
        );
        await expect(
          page.getByText("Did not finish", { exact: true }),
        ).toBeVisible();
        await expect(page.getByTestId("multiplayer-room-code")).toHaveText(
          code!,
        );
        await expect(
          page.locator("a[download], script[src*='adsbygoogle']"),
        ).toHaveCount(0);
      }
      await west.goto("/en/me/history");
      const saved = west.getByTestId("me-record");
      await expect(saved).toHaveCount(1);
      await saved.locator("summary").click();
      await expect(saved).toContainText("Players 4 · Tied for 2");
    } finally {
      await Promise.all(contexts.map((context) => context.close()));
    }
  });
}
