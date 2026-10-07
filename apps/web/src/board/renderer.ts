import type { PublicCell } from "@liar/protocol";
export interface BoardFrame {
  width: number;
  height: number;
  cells: readonly PublicCell[];
  contrast: "standard" | "high";
}
export interface BoardRenderer {
  draw(frame: BoardFrame): void;
  dispose(): void;
}
export type BoardRendererFactory = (
  host: HTMLElement,
) => Promise<BoardRenderer>;
export const CELL_PITCH = 48;
