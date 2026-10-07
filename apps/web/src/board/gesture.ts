export interface PointerSample {
  id: number;
  x: number;
  y: number;
  cell?: number;
  button: number;
  kind: string;
}
export interface GestureCallbacks {
  tap(cell: number): void;
  menu(cell: number): void;
  pan(x: number, y: number): void;
  zoom(ratio: number, x: number, y: number): void;
}
export interface TimerPort {
  schedule(callback: () => void, delay: number): unknown;
  cancel(handle: unknown): void;
}
export interface BoardGesture {
  down(sample: PointerSample): void;
  move(sample: PointerSample): void;
  up(id: number): void;
  cancel(id: number): void;
  dispose(): void;
}
export function createBoardGesture(
  callbacks: GestureCallbacks,
  timer: TimerPort,
): BoardGesture {
  type Point = PointerSample & {
    startX: number;
    startY: number;
    suppressed: boolean;
  };
  const points = new Map<number, Point>();
  let handle: unknown;
  const clear = () => {
    if (handle !== undefined) timer.cancel(handle);
    handle = undefined;
  };
  const distance = () => {
    const [a, b] = [...points.values()];
    return a && b ? Math.hypot(a.x - b.x, a.y - b.y) : 0;
  };
  return {
    down(sample) {
      if (sample.button !== 0 || points.has(sample.id)) return;
      points.set(sample.id, {
        ...sample,
        startX: sample.x,
        startY: sample.y,
        suppressed: false,
      });
      clear();
      if (points.size > 1) {
        for (const point of points.values()) point.suppressed = true;
      } else if (sample.kind !== "mouse" && sample.cell !== undefined) {
        handle = timer.schedule(() => {
          handle = undefined;
          const point = points.get(sample.id);
          if (point && !point.suppressed && points.size === 1) {
            point.suppressed = true;
            callbacks.menu(sample.cell!);
          }
        }, 450);
      }
    },
    move(sample) {
      const point = points.get(sample.id);
      if (!point) return;
      const previousDistance = distance();
      const dx = sample.x - point.x,
        dy = sample.y - point.y;
      point.x = sample.x;
      point.y = sample.y;
      if (points.size > 1) {
        clear();
        const [a, b] = [...points.values()];
        const nextDistance = distance();
        if (previousDistance > 0 && nextDistance > 0)
          callbacks.zoom(
            nextDistance / previousDistance,
            (a.x + b.x) / 2,
            (a.y + b.y) / 2,
          );
      } else if (
        Math.hypot(point.x - point.startX, point.y - point.startY) >= 8
      ) {
        clear();
        point.suppressed = true;
        callbacks.pan(-dx, -dy);
      }
    },
    up(id) {
      const point = points.get(id);
      if (!point) return;
      clear();
      points.delete(id);
      if (!point.suppressed && point.cell !== undefined)
        callbacks.tap(point.cell);
    },
    cancel(id) {
      clear();
      points.delete(id);
    },
    dispose() {
      clear();
      points.clear();
    },
  };
}
