import { expect, test, vi } from "vitest";
const mock = vi.hoisted(() => ({
  failInit: false,
  apps: [] as Array<{
    init: ReturnType<typeof vi.fn>;
    render: ReturnType<typeof vi.fn>;
    destroy: ReturnType<typeof vi.fn>;
    stage: { addChild: ReturnType<typeof vi.fn> };
    renderer: { resize: ReturnType<typeof vi.fn> };
    canvas: HTMLCanvasElement;
  }>,
  graphics: [] as Array<{ clear: ReturnType<typeof vi.fn> }>,
  texts: [] as Array<{ text: string }>,
}));
vi.mock("pixi.js", () => ({
  Application: class {
    canvas = document.createElement("canvas");
    init = vi.fn(async () => {
      if (mock.failInit) throw new Error("unavailable");
    });
    render = vi.fn();
    destroy = vi.fn();
    stage = { addChild: vi.fn() };
    renderer = { resize: vi.fn() };
    constructor() {
      mock.apps.push(this);
    }
  },
  Graphics: class {
    clear = vi.fn(() => this);
    roundRect = vi.fn(() => this);
    fill = vi.fn(() => this);
    stroke = vi.fn(() => this);
    position = { set: vi.fn() };
    destroy = vi.fn();
    constructor() {
      mock.graphics.push(this);
    }
  },
  Text: class {
    text = "";
    style = { fill: 0, fontSize: 22 };
    anchor = { set: vi.fn() };
    position = { set: vi.fn() };
    destroy = vi.fn();
    constructor() {
      mock.texts.push(this);
    }
  },
}));
import { createPixiBoard } from "./pixi";
test("reuses a fixed public-cell pool and renders only changed cells or theme", async () => {
  const host = document.createElement("div");
  const renderer = await createPixiBoard(host);
  const frame = {
    width: 2,
    height: 1,
    contrast: "standard" as const,
    cells: [
      { cell: 0, state: "closed" as const, number: null, flagged: true },
      { cell: 1, state: "safe" as const, number: 3, flagged: false },
    ],
  };
  renderer.draw(frame);
  const app = mock.apps.at(-1)!;
  expect(host.querySelector("canvas")).not.toBeNull();
  expect(app.init).toHaveBeenCalledWith(
    expect.objectContaining({
      autoStart: false,
      resolution: expect.any(Number),
    }),
  );
  expect(mock.graphics).toHaveLength(2);
  expect(mock.texts.map((t) => t.text)).toEqual(["⚑", "3"]);
  renderer.draw({ ...frame, cells: frame.cells.map((c) => ({ ...c })) });
  expect(app.render).toHaveBeenCalledTimes(1);
  renderer.draw({
    ...frame,
    cells: [frame.cells[0], { ...frame.cells[1], number: 4 }],
  });
  expect(mock.graphics[0].clear).toHaveBeenCalledTimes(1);
  expect(mock.graphics[1].clear).toHaveBeenCalledTimes(2);
  renderer.draw({ ...frame, contrast: "high" });
  expect(mock.graphics).toHaveLength(2);
  expect(app.render).toHaveBeenCalledTimes(3);
  renderer.dispose();
  renderer.draw(frame);
  renderer.dispose();
  expect(app.destroy).toHaveBeenCalledTimes(1);
  expect(app.render).toHaveBeenCalledTimes(3);
});

test("failed GPU initialization releases resources without attaching a canvas", async () => {
  const host = document.createElement("div");
  mock.failInit = true;
  try {
    await expect(createPixiBoard(host)).rejects.toThrow("unavailable");
  } finally {
    mock.failInit = false;
  }
  expect(host.children).toHaveLength(0);
  expect(mock.apps.at(-1)!.destroy).toHaveBeenCalledTimes(1);
});
