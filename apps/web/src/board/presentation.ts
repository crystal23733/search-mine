import type { PublicCell } from "@liar/protocol";
export function cellText(cell: PublicCell): string {
  return cell.flagged
    ? "⚑"
    : cell.state === "mine"
      ? "✹"
      : cell.state === "safe" && cell.number
        ? String(cell.number)
        : "";
}
export function coordinate(cell: number, width: number): string {
  let column = (cell % width) + 1;
  let name = "";
  while (column > 0) {
    column--;
    name = String.fromCharCode(65 + (column % 26)) + name;
    column = Math.floor(column / 26);
  }
  return `${name}${Math.floor(cell / width) + 1}`;
}
export function moveFocus(
  cell: number,
  width: number,
  height: number,
  key: string,
  control: boolean,
): number {
  const row = Math.floor(cell / width),
    column = cell % width;
  if (key === "ArrowLeft") return row * width + Math.max(0, column - 1);
  if (key === "ArrowRight")
    return row * width + Math.min(width - 1, column + 1);
  if (key === "ArrowUp") return Math.max(0, row - 1) * width + column;
  if (key === "ArrowDown")
    return Math.min(height - 1, row + 1) * width + column;
  if (key === "Home") return control ? 0 : row * width;
  if (key === "End")
    return control ? width * height - 1 : (row + 1) * width - 1;
  return cell;
}
