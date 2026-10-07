import { expect, test, vi } from "vitest";
import {
  createBoardGesture,
  type PointerSample,
  type TimerPort,
} from "./gesture";
function setup() {
  const callbacks = {
    tap: vi.fn(),
    menu: vi.fn(),
    pan: vi.fn(),
    zoom: vi.fn(),
  };
  let pending: (() => void) | undefined;
  const timer: TimerPort = {
    schedule: (fn, delay) => {
      expect(delay).toBe(450);
      pending = fn;
      return 1;
    },
    cancel: () => {
      pending = undefined;
    },
  };
  const gesture = createBoardGesture(callbacks, timer);
  const press = () => pending?.();
  const point = (patch: Partial<PointerSample> = {}): PointerSample => ({
    id: 1,
    x: 10,
    y: 10,
    cell: 17,
    button: 0,
    kind: "touch",
    ...patch,
  });
  return { callbacks, gesture, point, press };
}
test("tap sends one intent while a long press opens only the menu", () => {
  const { gesture, point, callbacks, press } = setup();
  gesture.down(point());
  gesture.up(1);
  expect(callbacks.tap.mock.calls).toEqual([[17]]);
  gesture.down(point());
  press();
  gesture.up(1);
  gesture.up(1);
  expect(callbacks.menu.mock.calls).toEqual([[17]]);
  expect(callbacks.tap).toHaveBeenCalledTimes(1);
});
test("pan cancels the long press and release never opens a cell", () => {
  const { gesture, point, callbacks, press } = setup();
  gesture.down(point());
  gesture.move(point({ x: 30, y: 40 }));
  press();
  gesture.up(1);
  expect(callbacks.pan.mock.calls).toEqual([[-20, -30]]);
  expect(callbacks.tap).not.toHaveBeenCalled();
  expect(callbacks.menu).not.toHaveBeenCalled();
});
test("pinch suppresses both releases, and cancelling/disposal clears pending menus", () => {
  const { gesture, point, callbacks, press } = setup();
  gesture.down(point());
  gesture.down(point({ id: 2, x: 30 }));
  gesture.move(point({ id: 2, x: 50 }));
  press();
  gesture.up(2);
  gesture.up(1);
  expect(callbacks.zoom.mock.calls).toEqual([[2, 30, 10]]);
  expect(callbacks.tap).not.toHaveBeenCalled();
  expect(callbacks.menu).not.toHaveBeenCalled();
  gesture.down(point());
  gesture.cancel(1);
  press();
  gesture.down(point());
  gesture.dispose();
  press();
  expect(callbacks.menu).not.toHaveBeenCalled();
});
test("mouse/right button, tiny movement and duplicate downs do not create extra intents", () => {
  const { gesture, point, callbacks, press } = setup();
  gesture.down(point({ button: 2 }));
  press();
  gesture.up(1);
  expect(callbacks.tap).not.toHaveBeenCalled();
  gesture.down(point({ kind: "mouse" }));
  gesture.down(point({ kind: "mouse" }));
  press();
  gesture.move(point({ x: 12 }));
  gesture.up(1);
  expect(callbacks.tap.mock.calls).toEqual([[17]]);
  expect(callbacks.menu).not.toHaveBeenCalled();
});
