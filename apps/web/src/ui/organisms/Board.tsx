/* oxlint-disable jsx-a11y/no-noninteractive-element-to-interactive-role -- The native table implements the interactive ARIA grid pattern with roving cell focus. */
import type { GameView, PublicAction } from "@liar/protocol";
import { useEffect, useRef, useState } from "preact/hooks";
import {
  CELL_PITCH,
  type BoardFrame,
  type BoardRenderer,
  type BoardRendererFactory,
} from "../../board/renderer";
import { createBoardGesture, type PointerSample } from "../../board/gesture";
import { coordinate, moveFocus } from "../../board/presentation";
import { Cell } from "../atoms/Cell";
import { Button } from "../atoms/Button";
import { Dialog } from "../atoms/Dialog";
import { useUi } from "../context";
export interface BoardProps {
  view: Pick<GameView, "rules" | "phase" | "own">;
  allowedModes?: readonly Mode[];
  createRenderer: BoardRendererFactory;
  onAction(action: PublicAction): void;
  disabled?: boolean;
}
type Mode = "open" | "flag" | "accuse";
export function Board({
  view,
  createRenderer,
  onAction,
  allowedModes = ["open", "flag", "accuse"],
  disabled = false,
}: BoardProps) {
  const { t, locale, preferences } = useUi();
  const { width, height } = view.rules.rules;
  const viewport = useRef<HTMLDivElement>(null),
    canvasHost = useRef<HTMLDivElement>(null);
  const buttons = useRef<Array<HTMLTableCellElement | null>>([]),
    renderer = useRef<BoardRenderer | undefined>(undefined);
  const [renderMode, setRenderMode] = useState("dom");
  const [selected, setSelected] = useState(0),
    [mode, setMode] = useState<Mode>("open");
  const currentMode = allowedModes.includes(mode)
    ? mode
    : (allowedModes[0] ?? "open");
  const [menu, setMenu] = useState<number | null>(null),
    [help, setHelp] = useState(false);
  const [zoom, setZoom] = useState<number>(preferences.zoom);
  const zoomRef = useRef(zoom),
    zoomFrame = useRef<number | undefined>(undefined);
  const pendingScroll = useRef<{ left: number; top: number } | undefined>(
    undefined,
  );
  const [visible, setVisible] = useState({
    left: 0,
    top: 0,
    width: 1,
    height: 1,
  });
  const locked = disabled || view.phase !== "playing" || view.own.stun_ms > 0;
  const latest = useRef({
    locked,
    onAction,
    mode: currentMode,
    allowedModes,
    view,
    contrast: preferences.contrast,
  });
  latest.current = {
    locked,
    onAction,
    mode: currentMode,
    allowedModes,
    view,
    contrast: preferences.contrast,
  };
  const measure = () => {
    const port = viewport.current;
    if (port)
      setVisible({
        left: port.scrollLeft,
        top: port.scrollTop,
        width: port.clientWidth,
        height: port.clientHeight,
      });
  };
  const select = (cell: number, focus = true) => {
    setSelected(cell);
    if (focus) {
      buttons.current[cell]?.focus({ preventScroll: true });
      buttons.current[cell]?.scrollIntoView?.({
        block: "nearest",
        inline: "nearest",
      });
    }
  };
  const emit = (cell: number, action: Mode = latest.current.mode) => {
    if (!latest.current.locked && latest.current.allowedModes.includes(action))
      latest.current.onAction({ type: action, cell });
  };
  const paint = (frame: BoardFrame): boolean => {
    try {
      renderer.current?.draw(frame);
      return true;
    } catch {
      const failed = renderer.current;
      renderer.current = undefined;
      try {
        failed?.dispose();
      } catch {
        /* DOM remains usable after GPU cleanup failure. */
      }
      setRenderMode("dom");
      return false;
    }
  };
  const applyZoom = (next: number, clientX?: number, clientY?: number) => {
    const port = viewport.current;
    const value = Math.max(1, Math.min(2, next));
    if (!port || value === zoomRef.current) return;
    const bounds = port.getBoundingClientRect();
    const x =
      clientX === undefined ? port.clientWidth / 2 : clientX - bounds.left;
    const y =
      clientY === undefined ? port.clientHeight / 2 : clientY - bounds.top;
    const previous = pendingScroll.current ?? {
      left: port.scrollLeft,
      top: port.scrollTop,
    };
    const ratio = value / zoomRef.current;
    pendingScroll.current = {
      left: (previous.left + x) * ratio - x,
      top: (previous.top + y) * ratio - y,
    };
    zoomRef.current = value;
    setZoom(value);
    if (zoomFrame.current !== undefined)
      cancelAnimationFrame(zoomFrame.current);
    zoomFrame.current = requestAnimationFrame(() => {
      const offset = pendingScroll.current;
      if (offset && viewport.current) {
        viewport.current.scrollLeft = offset.left;
        viewport.current.scrollTop = offset.top;
      }
      pendingScroll.current = undefined;
      zoomFrame.current = undefined;
      measure();
    });
  };
  const gesture = useRef<ReturnType<typeof createBoardGesture> | undefined>(
    undefined,
  );
  if (!gesture.current)
    gesture.current = createBoardGesture(
      {
        tap: (cell) => {
          select(cell);
          emit(cell);
        },
        menu: (cell) => {
          select(cell);
          if (!latest.current.locked) setMenu(cell);
        },
        pan: (x, y) => {
          if (viewport.current) {
            viewport.current.scrollLeft += x;
            viewport.current.scrollTop += y;
            measure();
          }
        },
        zoom: (ratio, x, y) => applyZoom(zoomRef.current * ratio, x, y),
      },
      {
        schedule: (fn, delay) => setTimeout(fn, delay),
        cancel: (id) => clearTimeout(id as ReturnType<typeof setTimeout>),
      },
    );
  useEffect(() => {
    let active = true;
    const host = canvasHost.current!;
    void createRenderer(host)
      .then((adapter) => {
        if (!active) {
          adapter.dispose();
          return;
        }
        renderer.current = adapter;
        const current = latest.current;
        if (
          paint({
            width: current.view.rules.rules.width,
            height: current.view.rules.rules.height,
            cells: current.view.own.cells,
            contrast: current.contrast,
          })
        )
          setRenderMode("pixi");
      })
      .catch(() => {
        if (active) setRenderMode("dom");
      });
    return () => {
      active = false;
      renderer.current?.dispose();
      renderer.current = undefined;
    };
  }, [createRenderer]);
  useEffect(() => {
    paint({
      width,
      height,
      cells: view.own.cells,
      contrast: preferences.contrast,
    });
  }, [width, height, view.own.cells, preferences.contrast]);
  useEffect(() => {
    applyZoom(preferences.zoom);
  }, [preferences.zoom]);
  useEffect(() => {
    measure();
    const observer =
      typeof ResizeObserver === "undefined"
        ? undefined
        : new ResizeObserver(measure);
    if (viewport.current) observer?.observe(viewport.current);
    window.addEventListener("resize", measure);
    return () => {
      observer?.disconnect();
      window.removeEventListener("resize", measure);
      gesture.current?.dispose();
      if (zoomFrame.current !== undefined)
        cancelAnimationFrame(zoomFrame.current);
    };
  }, []);
  const sample = (event: PointerEvent): PointerSample => {
    const target = (event.target as HTMLElement).closest<HTMLElement>(
      "[data-cell]",
    );
    return {
      id: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      button: event.button,
      kind: event.pointerType,
      ...(target ? { cell: Number(target.dataset.cell) } : {}),
    };
  };
  const label = (cell: GameView["own"]["cells"][number]) => {
    const status =
      cell.state === "safe"
        ? t("board.number", { number: cell.number ?? 0 })
        : t(cell.state === "mine" ? "board.mine" : "board.closed");
    return `${coordinate(cell.cell, width)}, ${status}${cell.flagged ? `, ${t("board.flagged")}` : ""}`;
  };
  const key = (event: KeyboardEvent, cell: number) => {
    if (
      [
        "ArrowLeft",
        "ArrowRight",
        "ArrowUp",
        "ArrowDown",
        "Home",
        "End",
      ].includes(event.key)
    ) {
      event.preventDefault();
      select(moveFocus(cell, width, height, event.key, event.ctrlKey));
    } else if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      emit(cell);
    } else if (
      ["a", "f", "o"].includes(event.key.toLowerCase()) &&
      !event.ctrlKey &&
      !event.metaKey
    ) {
      event.preventDefault();
      const next =
        event.key.toLowerCase() === "a"
          ? "accuse"
          : event.key.toLowerCase() === "f"
            ? "flag"
            : "open";
      if (allowedModes.includes(next)) setMode(next);
    } else if (event.key === "F10" && event.shiftKey) {
      event.preventDefault();
      if (!locked) setMenu(cell);
    }
  };
  const totalWidth = width * CELL_PITCH * zoom,
    totalHeight = height * CELL_PITCH * zoom;
  const menuCell =
    menu === null
      ? undefined
      : view.own.cells.find((cell) => cell.cell === menu);
  return (
    <section class="game-board" aria-label={t("board.title")}>
      <div class="board-tools">
        <p aria-live="polite">
          {locked
            ? view.own.stun_ms > 0
              ? t("board.stun", {
                  seconds: new Intl.NumberFormat(locale).format(
                    Math.ceil(view.own.stun_ms / 1000),
                  ),
                })
              : t("board.paused")
            : t("board.ready")}
        </p>
        <div class="zoom-controls">
          <Button
            aria-label={t("board.zoomOut")}
            disabled={zoom <= 1}
            onClick={() => applyZoom(zoom - 0.25)}
          >
            −
          </Button>
          <output aria-label={t("board.zoom")}>
            {new Intl.NumberFormat(locale, { style: "percent" }).format(zoom)}
          </output>
          <Button
            aria-label={t("board.zoomIn")}
            disabled={zoom >= 2}
            onClick={() => applyZoom(zoom + 0.25)}
          >
            +
          </Button>
        </div>
      </div>
      <div
        ref={viewport}
        class="board-viewport"
        onScroll={measure}
        onPointerDown={(event) => {
          if (event.button === 0) {
            event.currentTarget.setPointerCapture?.(event.pointerId);
            gesture.current!.down(sample(event));
          }
        }}
        onPointerMove={(event) => gesture.current!.move(sample(event))}
        onPointerUp={(event) => gesture.current!.up(event.pointerId)}
        onPointerCancel={(event) => gesture.current!.cancel(event.pointerId)}
        onLostPointerCapture={(event) =>
          gesture.current!.cancel(event.pointerId)
        }
      >
        <div
          class="board-surface"
          style={{ width: `${totalWidth}px`, height: `${totalHeight}px` }}
        >
          <div class="board-pixi-host" ref={canvasHost} />
          <table
            role="grid"
            aria-label={t("board.title")}
            aria-rowcount={height}
            aria-colcount={width}
            aria-readonly={locked}
            data-renderer={renderMode}
            class="board-grid"
            style={{ "--cell-pitch": `${CELL_PITCH * zoom}px` }}
          >
            <tbody>
              {Array.from({ length: height }, (_, row) => (
                <tr class="board-row" key={row}>
                  {view.own.cells
                    .slice(row * width, (row + 1) * width)
                    .map((cell) => (
                      <Cell
                        key={cell.cell}
                        cell={cell}
                        label={label(cell)}
                        selected={selected === cell.cell}
                        locked={locked}
                        ref={(element) => {
                          buttons.current[cell.cell] = element;
                        }}
                        onFocus={() => setSelected(cell.cell)}
                        onKeyDown={(event) => key(event, cell.cell)}
                        onClick={(event) => {
                          if (event.detail === 0) {
                            select(cell.cell);
                            emit(cell.cell);
                          }
                        }}
                        onContextMenu={(event) => {
                          event.preventDefault();
                          select(cell.cell);
                          if (event.button === 2) emit(cell.cell, "flag");
                          else if (!locked) setMenu(cell.cell);
                        }}
                      />
                    ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
      <div class="board-bottom">
        <fieldset class="board-modes">
          <legend class="visually-hidden">{t("board.mode")}</legend>
          {allowedModes.map((value) => (
            <Button
              key={value}
              aria-pressed={currentMode === value}
              variant={currentMode === value ? "primary" : "secondary"}
              onClick={() => setMode(value)}
            >
              {t(`board.${value}`)}
            </Button>
          ))}
          <Button onClick={() => setHelp(true)}>{t("board.help")}</Button>
        </fieldset>
        <button
          type="button"
          class="board-map"
          aria-label={t("board.map")}
          onClick={(event) => {
            const bounds = event.currentTarget.getBoundingClientRect(),
              port = viewport.current;
            if (port) {
              port.scrollLeft =
                ((event.clientX - bounds.left) / bounds.width) * totalWidth -
                port.clientWidth / 2;
              port.scrollTop =
                ((event.clientY - bounds.top) / bounds.height) * totalHeight -
                port.clientHeight / 2;
              measure();
            }
          }}
        >
          <span
            aria-hidden="true"
            class="map-cells"
            style={{ gridTemplateColumns: `repeat(${width}, 1fr)` }}
          >
            {view.own.cells.map((cell) => (
              <i key={cell.cell} data-state={cell.state} />
            ))}
          </span>
          <span
            aria-hidden="true"
            class="map-window"
            style={{
              left: `${(visible.left / totalWidth) * 100}%`,
              top: `${(visible.top / totalHeight) * 100}%`,
              width: `${Math.min(1, visible.width / totalWidth) * 100}%`,
              height: `${Math.min(1, visible.height / totalHeight) * 100}%`,
            }}
          />
        </button>
      </div>
      <p class="muted">
        {t("board.selected", { cell: coordinate(selected, width) })} ·{" "}
        {t(`board.${currentMode}`)}
      </p>
      <Dialog
        open={menu !== null || help}
        onClose={() => {
          setMenu(null);
          setHelp(false);
        }}
        title={
          help ? t("board.help") : menuCell ? label(menuCell) : t("board.title")
        }
      >
        {help ? (
          <p>
            {t(
              allowedModes.includes("accuse") ? "board.helpText" : "daily.help",
            )}
          </p>
        ) : (
          menuCell && (
            <div class="cell-menu">
              {allowedModes.map((value) => (
                <Button
                  key={value}
                  disabled={
                    locked ||
                    (value === "accuse"
                      ? menuCell.state !== "safe" || !menuCell.number
                      : menuCell.state !== "closed")
                  }
                  onClick={() => {
                    setMenu(null);
                    emit(menuCell.cell, value);
                  }}
                >
                  {t(`board.${value}`)}
                </Button>
              ))}
            </div>
          )
        )}
      </Dialog>
    </section>
  );
}
