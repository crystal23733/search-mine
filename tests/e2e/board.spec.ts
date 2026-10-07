import { expect, test, type Page } from "@playwright/test";
import { readFileSync, mkdirSync } from "node:fs";
import type { GameView, PublicAction } from "../../packages/protocol/src/index";
const source = JSON.parse(
  readFileSync(
    new URL("../fixtures/native-wasm.json", import.meta.url),
    "utf8",
  ),
) as { cases: Array<{ initial: GameView }> };
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() =>
    localStorage.setItem("liar.tutorial.v1", '"skipped"'),
  );
});
declare global {
  interface Window {
    __board: {
      actions: PublicAction[];
      view: GameView;
      update(view: GameView): void;
    };
  }
}
async function mount(page: Page) {
  await page.goto("/en/");
  await page.evaluate(async (initial) => {
    const path = "/test/browser-board.ts";
    const { mountBoard } = await import(/* @vite-ignore */ path);
    window.__board = await mountBoard(initial);
  }, source.cases[0].initial);
  await expect(page.getByRole("grid")).toHaveAttribute("data-renderer", "pixi");
}
test("public canvas and DOM agree, keyboard dispatches once and stun keeps synchronization", async ({
  page,
}) => {
  await mount(page);
  const cells = page.getByRole("gridcell");
  await expect(cells).toHaveCount(256);
  await expect(page.locator("canvas")).toHaveAttribute("aria-hidden", "true");
  const geometry = await page.locator(".board-surface").evaluate((surface) => {
    const canvas = surface.querySelector("canvas")!.getBoundingClientRect();
    const cell = surface.querySelector("td")!.getBoundingClientRect();
    return {
      width: surface.getBoundingClientRect().width,
      height: surface.getBoundingClientRect().height,
      canvasWidth: canvas.width,
      canvasHeight: canvas.height,
      cellWidth: cell.width,
      cellHeight: cell.height,
    };
  });
  expect(geometry).toEqual({
    width: 768,
    height: 768,
    canvasWidth: 768,
    canvasHeight: 768,
    cellWidth: 44,
    cellHeight: 44,
  });
  expect(
    await cells.evaluateAll(
      (nodes) => nodes.filter((n) => n.getAttribute("tabindex") === "0").length,
    ),
  ).toBe(1);
  await cells.first().focus();
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("Enter");
  await page.keyboard.press("f");
  await page.keyboard.press("Space");
  expect(await page.evaluate(() => window.__board.actions)).toEqual([
    { type: "open", cell: 1 },
    { type: "flag", cell: 1 },
  ]);
  await page.evaluate(() =>
    window.__board.update({
      ...window.__board.view,
      own: { ...window.__board.view.own, stun_ms: 2000 },
    }),
  );
  await page.keyboard.press("Enter");
  await expect(page.getByRole("grid")).toHaveAttribute("aria-readonly", "true");
  expect(await page.evaluate(() => window.__board.actions.length)).toBe(2);
  await page.evaluate(() => {
    const view = window.__board.view;
    const cells = view.own.cells.map((cell) =>
      cell.cell === 1 ? { ...cell, state: "safe" as const, number: 3 } : cell,
    );
    window.__board.update({ ...view, own: { ...view.own, cells } });
  });
  await expect(cells.nth(1)).toHaveAttribute("aria-label", "B1, number 3");
  expect(await cells.nth(1).getAttribute("aria-label")).not.toMatch(
    /truth|lie|seed|overlay/i,
  );
});
test("real touch long press, drag and pinch never produce a trailing open command", async ({
  page,
  context,
}) => {
  await mount(page);
  const client = await context.newCDPSession(page);
  const bounds = (await page.getByRole("gridcell").first().boundingBox())!;
  const x = bounds.x + bounds.width / 2,
    y = bounds.y + bounds.height / 2;
  const touch = async (
    type: "touchStart" | "touchMove" | "touchEnd" | "touchCancel",
    points: Array<{ x: number; y: number; id: number }>,
  ) => client.send("Input.dispatchTouchEvent", { type, touchPoints: points });
  await touch("touchStart", [{ x, y, id: 1 }]);
  await expect(page.getByRole("dialog")).toBeVisible();
  await touch("touchEnd", []);
  await page.keyboard.press("Escape");
  await expect(page.getByRole("gridcell").first()).toBeFocused();
  expect(await page.evaluate(() => window.__board.actions)).toEqual([]);
  await touch("touchStart", [{ x: x + 100, y: y + 100, id: 1 }]);
  await touch("touchMove", [{ x: x + 30, y: y + 30, id: 1 }]);
  await touch("touchEnd", []);
  expect(await page.evaluate(() => window.__board.actions)).toEqual([]);
  const before = await page
    .getByRole("status", { name: "Board zoom" })
    .textContent();
  await touch("touchStart", [
    { x: x + 60, y: y + 60, id: 1 },
    { x: x + 140, y: y + 60, id: 2 },
  ]);
  await touch("touchMove", [
    { x: x + 30, y: y + 60, id: 1 },
    { x: x + 180, y: y + 60, id: 2 },
  ]);
  await touch("touchEnd", []);
  await expect(page.getByRole("status", { name: "Board zoom" })).not.toHaveText(
    before!,
  );
  expect(await page.evaluate(() => window.__board.actions)).toEqual([]);
  await client.detach();
});
test("zoom and keyboard scrolling remain inside the viewport across sizes", async ({
  page,
}, info) => {
  await mount(page);
  for (const width of [360, 390, 768, 1024, 1440]) {
    await page.setViewportSize({ width, height: 1000 });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
  }
  await page.getByRole("button", { name: "Zoom in" }).click();
  await expect(page.getByRole("status", { name: "Board zoom" })).toHaveText(
    "125%",
  );
  await page.setViewportSize({ width: 390, height: 1000 });
  await page.getByRole("gridcell").first().focus();
  await page.keyboard.press("Control+End");
  await expect(page.getByRole("gridcell").last()).toBeFocused();
  expect(
    await page.locator(".board-viewport").evaluate((port) => port.scrollLeft),
  ).toBeGreaterThan(0);
  if (process.env.LIAR_CAPTURE_DESIGN) {
    mkdirSync(".tmp/screenshots", { recursive: true });
    await page.setViewportSize({
      width: info.project.name === "mobile" ? 390 : 1440,
      height: 1000,
    });
    await page.getByRole("button", { name: "Zoom out" }).click();
    await page.getByRole("gridcell").first().focus();
    await page.keyboard.press("Control+Home");
    await page.screenshot({
      path: `.tmp/screenshots/11-board-${info.project.name}.png`,
      fullPage: true,
    });
  }
});
