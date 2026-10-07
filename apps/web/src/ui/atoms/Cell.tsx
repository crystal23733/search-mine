/* oxlint-disable jsx-a11y/no-redundant-roles -- Explicit gridcell preserves the interactive role across DOM accessibility consumers that otherwise expose td as cell. */
import type { JSX } from "preact";
import type { PublicCell } from "@liar/protocol";
import { cellText } from "../../board/presentation";
export function Cell({
  cell,
  label,
  selected,
  locked,
  ...events
}: {
  cell: PublicCell;
  label: string;
  selected: boolean;
  locked: boolean;
} & Pick<
  JSX.IntrinsicElements["td"],
  "onClick" | "onKeyDown" | "onContextMenu" | "onFocus" | "ref"
>) {
  return (
    <td
      {...events}
      role="gridcell"
      data-cell={cell.cell}
      data-state={cell.state}
      data-number={cell.state === "safe" ? (cell.number ?? "") : ""}
      class="board-cell"
      aria-label={label}
      aria-selected={selected}
      aria-disabled={locked}
      tabIndex={selected ? 0 : -1}
    >
      <span aria-hidden="true">{cellText(cell)}</span>
    </td>
  );
}
