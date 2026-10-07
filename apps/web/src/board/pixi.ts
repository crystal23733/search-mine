import { Application, Graphics, Text } from "pixi.js";
import { COLORS, HIGH_CONTRAST_COLORS } from "@liar/design-tokens";
import { CELL_PITCH, type BoardRenderer } from "./renderer";
import { cellText } from "./presentation";
export async function createPixiBoard(
  host: HTMLElement,
): Promise<BoardRenderer> {
  const app = new Application();
  try {
    await app.init({
      width: 1,
      height: 1,
      autoStart: false,
      antialias: true,
      backgroundAlpha: 0,
      resolution: Math.min(2, Math.max(1, globalThis.devicePixelRatio || 1)),
      autoDensity: true,
      preference: "webgl",
    });
  } catch (error) {
    try {
      app.destroy(true, { children: true });
    } catch {
      /* Initialization may fail before a renderer exists. */
    }
    throw error;
  }
  const canvas = app.canvas;
  canvas.setAttribute("aria-hidden", "true");
  canvas.className = "board-canvas";
  host.append(canvas);
  let width = 0,
    height = 0,
    contrast = "";
  let pool: Array<{ graphic: Graphics; text: Text; signature: string }> = [];
  let disposed = false;
  return {
    draw(frame) {
      if (disposed) return;
      const resized = width !== frame.width || height !== frame.height;
      if (resized) {
        for (const display of pool) {
          display.graphic.destroy();
          display.text.destroy();
        }
        width = frame.width;
        height = frame.height;
        app.renderer.resize(width * CELL_PITCH, height * CELL_PITCH);
        pool = Array.from({ length: width * height }, (_, cell) => {
          const graphic = new Graphics();
          const text = new Text({
            text: "",
            style: {
              fontFamily: "system-ui, sans-serif",
              fontSize: 24,
              fontWeight: "600",
              fill: COLORS.text,
            },
          });
          graphic.position.set(
            (cell % width) * CELL_PITCH,
            Math.floor(cell / width) * CELL_PITCH,
          );
          text.anchor.set(0.5);
          text.position.set(
            ((cell % width) + 0.5) * CELL_PITCH,
            (Math.floor(cell / width) + 0.5) * CELL_PITCH,
          );
          app.stage.addChild(graphic, text);
          return { graphic, text, signature: "" };
        });
      }
      const themeChanged = contrast !== frame.contrast;
      const colors = frame.contrast === "high" ? HIGH_CONTRAST_COLORS : COLORS;
      let dirty = resized || themeChanged;
      for (const cell of frame.cells) {
        const display = pool[cell.cell];
        if (!display) continue;
        const signature = `${cell.state}:${cell.number}:${cell.flagged}`;
        if (signature === display.signature && !themeChanged) continue;
        dirty = true;
        display.signature = signature;
        display.graphic
          .clear()
          .roundRect(2, 2, CELL_PITCH - 4, CELL_PITCH - 4, 8)
          .fill(cell.state === "closed" ? colors.closed : colors.background)
          .stroke({ width: 1, color: colors.border });
        display.text.text = cellText(cell);
        display.text.style.fill = cell.flagged
          ? colors.accent
          : cell.state === "mine"
            ? colors.danger
            : cell.number && cell.state === "safe"
              ? (colors[`number${cell.number}` as "number1"] ?? colors.text)
              : colors.text;
      }
      contrast = frame.contrast;
      if (dirty) app.render();
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      app.destroy(true, { children: true });
      canvas.remove();
      pool = [];
    },
  };
}
