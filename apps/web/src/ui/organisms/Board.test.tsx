import { dailyTestPorts } from "../../../test/daily-ports";
import { createLearning } from "../../services/learning";
import { fireEvent, render, screen, waitFor } from "@testing-library/preact";
import { expect, test, vi } from "vitest";
import { DEFAULT_RULES, type GameView } from "@liar/protocol";
import { Board } from "./Board";
import { Ui, type UiContext } from "../context";
import { createI18n } from "../../services/i18n";
import {
  createPreferences,
  DEFAULT_PREFERENCES,
} from "../../services/preferences";
import { createNavigation } from "../../services/navigation";
import type { BoardRenderer } from "../../board/renderer";
function fixture(): GameView {
  return {
    v: 1,
    rules: {
      ...DEFAULT_RULES,
      rules: { ...DEFAULT_RULES.rules, width: 4, height: 3 },
    },
    revision: 1,
    phase: "playing",
    countdown_ms: 0,
    remaining_ms: 10000,
    own: {
      cells: Array.from({ length: 12 }, (_, cell) => ({
        cell,
        state: cell === 2 ? "safe" : "closed",
        number: cell === 2 ? 3 : null,
        flagged: cell === 3,
      })),
      gauge: 0,
      stun_ms: 0,
      history: [],
      stats: { mistakes: 0, accusation_attempts: 0, correct_accusations: 0 },
    },
    opponent: { opened_safe: 0, stun_ms: 0 },
    result: null,
  };
}
async function setup(
  factory?: () => Promise<BoardRenderer>,
  allowedModes?: ("open" | "flag")[],
) {
  const adapter = await createI18n("en");
  const ui: UiContext = {
    locale: "en",
    t: (key, values) => adapter.t("en", key, values),
    preferences: DEFAULT_PREFERENCES,
    services: {
      ...dailyTestPorts(),
      i18n: adapter,
      preferences: createPreferences(),
      navigation: createNavigation(window),
      learning: createLearning(),
      trainingCore: async () => {
        throw new Error("unavailable");
      },
      practiceCore: async () => {
        throw new Error("unavailable");
      },
      boardRenderer: async () => {
        throw new Error("unavailable");
      },
    },
    selectLocale() {},
  };
  const draw = vi.fn(),
    dispose = vi.fn(),
    onAction = vi.fn();
  const createRenderer = factory ?? (async () => ({ draw, dispose }));
  const view = fixture();
  const tree = (next: GameView = view) => (
    <Ui.Provider value={ui}>
      <Board
        view={next}
        onAction={onAction}
        createRenderer={createRenderer}
        allowedModes={allowedModes}
      />
    </Ui.Provider>
  );
  const mounted = render(tree());
  return { ...mounted, tree, view, draw, dispose, onAction, ui };
}
test("solo board restricts mode buttons, keyboard and cell menu to the Rust action allowlist", async () => {
  const { onAction } = await setup(undefined, ["open", "flag"]);
  expect(
    screen.queryByRole("button", { name: "Accuse", exact: true }),
  ).toBeNull();
  const cell = screen.getAllByRole("gridcell")[2];
  fireEvent.keyDown(cell, { key: "a" });
  fireEvent.keyDown(cell, { key: "Enter" });
  expect(onAction).toHaveBeenCalledExactlyOnceWith({ type: "open", cell: 2 });
  fireEvent.keyDown(cell, { key: "F10", shiftKey: true });
  expect(screen.getByRole("dialog").textContent).not.toContain("Accuse");
});
test("DOM exposes only public coordinates/numbers/flags with one roving tab stop", async () => {
  const { draw } = await setup();
  const cells = screen.getAllByRole("gridcell");
  expect(cells).toHaveLength(12);
  expect(cells.filter((c) => c.tabIndex === 0)).toHaveLength(1);
  expect(
    screen.getByRole("gridcell", { name: "C1, number 3" }).textContent,
  ).toContain("3");
  expect(
    screen.getByRole("gridcell", { name: "D1, closed, flag" }).isConnected,
  ).toBe(true);
  expect(cells.map((c) => c.getAttribute("aria-label")).join(" ")).not.toMatch(
    /truth|lie|seed|overlay/i,
  );
  await waitFor(() => expect(draw).toHaveBeenCalled());
});
test("keyboard selection and mode keys dispatch public intents without mutating the view", async () => {
  const { onAction, view } = await setup();
  const before = JSON.stringify(view);
  const first = screen.getAllByRole("gridcell")[0];
  first.focus();
  fireEvent.keyDown(first, { key: "ArrowRight" });
  const second = screen.getAllByRole("gridcell")[1];
  expect(document.activeElement).toBe(second);
  fireEvent.keyDown(second, { key: "Enter" });
  fireEvent.keyDown(second, { key: "f" });
  fireEvent.keyDown(second, { key: " " });
  fireEvent.keyDown(second, { key: "ArrowRight" });
  const third = screen.getAllByRole("gridcell")[2];
  fireEvent.keyDown(third, { key: "a" });
  fireEvent.keyDown(third, { key: "Enter" });
  expect(onAction.mock.calls).toEqual([
    [{ type: "open", cell: 1 }],
    [{ type: "flag", cell: 1 }],
    [{ type: "accuse", cell: 2 }],
  ]);
  expect(JSON.stringify(view)).toBe(before);
});
test("stun/phase prevents commands while navigation and public updates continue", async () => {
  const { view, rerender, tree, onAction, draw } = await setup();
  const stunned = { ...view, own: { ...view.own, stun_ms: 2000 } };
  rerender(tree(stunned));
  const cells = screen.getAllByRole("gridcell");
  fireEvent.click(cells[0]);
  fireEvent.keyDown(cells[0], { key: "Enter" });
  fireEvent.keyDown(cells[0], { key: "ArrowDown" });
  expect(document.activeElement).toBe(cells[4]);
  expect(onAction).not.toHaveBeenCalled();
  expect(screen.getByRole("grid").getAttribute("aria-readonly")).toBe("true");
  const changed = {
    ...view,
    phase: "finished" as const,
    own: {
      ...view.own,
      cells: view.own.cells.map((c) =>
        c.cell === 2 ? { ...c, number: 4 } : c,
      ),
    },
  };
  rerender(tree(changed));
  fireEvent.click(cells[1]);
  expect(
    screen.getByRole("gridcell", { name: "C1, number 4" }).isConnected,
  ).toBe(true);
  await waitFor(() =>
    expect(draw).toHaveBeenLastCalledWith(
      expect.objectContaining({ cells: changed.own.cells }),
    ),
  );
  expect(onAction).not.toHaveBeenCalled();
});
test("renderer rejection keeps an operable DOM board and delayed initialization is disposed after unmount", async () => {
  const failed = await setup(async () => {
    throw new Error("no GPU");
  });
  await waitFor(() =>
    expect(screen.getByRole("grid").getAttribute("data-renderer")).toBe("dom"),
  );
  fireEvent.click(screen.getAllByRole("gridcell")[0]);
  expect(failed.onAction).toHaveBeenCalledWith({ type: "open", cell: 0 });
  failed.unmount();
  let resolve: (renderer: BoardRenderer) => void = () => {};
  const dispose = vi.fn();
  const delayed = await setup(
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  delayed.unmount();
  resolve({ draw: vi.fn(), dispose });
  await waitFor(() => expect(dispose).toHaveBeenCalledTimes(1));
});

test("cell menu and help close without extra commands and restore keyboard focus", async () => {
  const { onAction } = await setup();
  const cell = screen.getAllByRole("gridcell")[2];
  cell.focus();
  fireEvent.keyDown(cell, { key: "F10", shiftKey: true });
  const dialog = await screen.findByRole("dialog", { name: "C1, number 3" });
  fireEvent.click(dialog.querySelectorAll("button")[2]);
  expect(onAction).toHaveBeenCalledExactlyOnceWith({ type: "accuse", cell: 2 });
  await waitFor(() => expect(document.activeElement).toBe(cell));
  fireEvent.click(screen.getByRole("button", { name: "Help" }));
  expect(screen.getByRole("dialog", { name: "Help" }).textContent).toContain(
    "pinch to zoom",
  );
  fireEvent(
    screen.getByRole("dialog", { name: "Help" }),
    new Event("cancel", { cancelable: true }),
  );
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(onAction).toHaveBeenCalledTimes(1);
});

test("zoom, minimap, context flag and pointer cancellation preserve command count", async () => {
  const { onAction, ui, rerender, tree } = await setup();
  fireEvent.click(screen.getByRole("button", { name: "Zoom in" }));
  await waitFor(() =>
    expect(screen.getByRole("status", { name: "Board zoom" }).textContent).toBe(
      "125%",
    ),
  );
  fireEvent.click(screen.getByRole("button", { name: "Zoom out" }));
  fireEvent.click(screen.getByRole("button", { name: "Move board viewport" }), {
    clientX: 20,
    clientY: 20,
  });
  const cell = screen.getAllByRole("gridcell")[0];
  fireEvent.contextMenu(cell, { button: 2 });
  fireEvent.click(cell, { detail: 1 });
  expect(onAction).toHaveBeenCalledExactlyOnceWith({ type: "flag", cell: 0 });
  const pointer = (type: string, x: number) => {
    const event = new MouseEvent(type, {
      bubbles: true,
      button: 0,
      clientX: x,
      clientY: 20,
    });
    Object.defineProperties(event, {
      pointerId: { value: 1 },
      pointerType: { value: "touch" },
    });
    fireEvent(cell, event);
  };
  pointer("pointerdown", 100);
  pointer("pointermove", 50);
  pointer("pointercancel", 50);
  pointer("pointerup", 50);
  expect(onAction).toHaveBeenCalledTimes(1);
  ui.preferences = { ...ui.preferences, contrast: "high", zoom: 1.5 };
  rerender(tree());
  fireEvent.scroll(cell.closest(".board-viewport")!);
  fireEvent(window, new Event("resize"));
  await waitFor(() =>
    expect(screen.getByRole("status", { name: "Board zoom" }).textContent).toBe(
      "150%",
    ),
  );
});

test("render failure releases the adapter immediately and retains DOM commands", async () => {
  const draw = vi.fn(() => {
      throw new Error("context lost");
    }),
    dispose = vi.fn();
  const { onAction } = await setup(async () => ({ draw, dispose }));
  await waitFor(() => expect(draw).toHaveBeenCalled());
  expect(dispose).toHaveBeenCalledTimes(1);
  expect(screen.getByRole("grid").getAttribute("data-renderer")).toBe("dom");
  fireEvent.click(screen.getAllByRole("gridcell")[0]);
  expect(onAction).toHaveBeenCalledExactlyOnceWith({ type: "open", cell: 0 });
});
