import { expect, test, devices } from "@playwright/test";
import { readFileSync, mkdirSync, writeFileSync } from "node:fs";
import type { GameView } from "../../packages/protocol/src/index";
const source = JSON.parse(
  readFileSync(
    new URL("../fixtures/native-wasm.json", import.meta.url),
    "utf8",
  ),
) as { cases: Array<{ initial: GameView }> };
// Run only after the functional projects, with one GPU context at a time.
test.describe.configure({ mode: "serial" });
for (const [mode, device] of [
  ["chromium", "Desktop Chrome"],
  ["mobile", "Pixel 7"],
] as const) {
  test(`${mode} manual dirty renderer records development-browser timings`, async ({
    browser,
  }) => {
    const context = await browser.newContext({
      ...devices[device],
      baseURL: "http://127.0.0.1:5173",
    });
    const page = await context.newPage();
    await page.addInitScript(() =>
      localStorage.setItem("liar.tutorial.v1", '"skipped"'),
    );
    try {
      // This benchmark measures one renderer, matching the application's single board.
      test.setTimeout(60_000);
      await page.goto("/en/settings");
      const measurement = await page.evaluate(async (initial) => {
        const path = "/src/board/pixi.ts";
        const { createPixiBoard } = await import(/* @vite-ignore */ path);
        const host = document.createElement("div");
        document.body.append(host);
        const renderer = await createPixiBoard(host);
        const frame = {
          width: initial.rules.rules.width,
          height: initial.rules.rules.height,
          cells: initial.own.cells,
          contrast: "standard",
        };
        renderer.draw(frame);
        const renderMs: number[] = [],
          frameMs: number[] = [];
        let previous = await new Promise<number>((resolve) =>
          requestAnimationFrame(resolve),
        );
        for (let index = 0; index < 120; index++) {
          const time = await new Promise<number>((resolve) =>
            requestAnimationFrame(resolve),
          );
          frameMs.push(time - previous);
          previous = time;
          const start = performance.now();
          renderer.draw({
            ...frame,
            cells: frame.cells.map((cell: GameView["own"]["cells"][number]) =>
              cell.cell === 1 ? { ...cell, flagged: index % 2 === 0 } : cell,
            ),
          });
          renderMs.push(performance.now() - start);
        }
        renderer.dispose();
        host.remove();
        return {
          renderMs,
          frameMs,
          userAgent: navigator.userAgent,
          dpr: devicePixelRatio,
        };
      }, source.cases[0].initial);
      expect(measurement.renderMs).toHaveLength(120);
      expect(
        measurement.renderMs.every((n) => Number.isFinite(n) && n >= 0),
      ).toBe(true);
      mkdirSync(".tmp/board-metrics", { recursive: true });
      writeFileSync(
        `.tmp/board-metrics/${mode}.json`,
        JSON.stringify(measurement),
      );
      await test.info().attach("board-development-timings", {
        body: JSON.stringify(measurement),
        contentType: "application/json",
      });
    } finally {
      await context.close();
    }
  });
}
