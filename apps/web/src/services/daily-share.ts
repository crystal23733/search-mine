import type { DailyView } from "@liar/protocol";
import { COLORS } from "@liar/design-tokens";
export interface DailyShare {
  date: string;
  version: number;
  completed: boolean;
  opened_safe: number;
  safe_total: number;
  elapsed_ms: number;
  mistakes: number;
  trust: "unverified";
}
export interface SharePort {
  copy(text: string): Promise<void>;
  nativeShare(text: string): Promise<void>;
  download(card: DailyShare, text: string): Promise<void>;
}
export function dailyShare(view: DailyView): DailyShare {
  const rules = view.rules.rules;
  return {
    date: view.metadata.date,
    version: view.metadata.seed_version,
    completed: view.completed,
    opened_safe: view.own.cells.filter((cell) => cell.state === "safe").length,
    safe_total: rules.width * rules.height - rules.mines,
    elapsed_ms: view.elapsed_ms,
    mistakes: view.own.stats.mistakes,
    trust: "unverified",
  };
}
export function shareMosaic(card: DailyShare): string[] {
  const filled = Math.floor(
    32 *
      Math.min(1, Math.max(0, card.opened_safe / Math.max(1, card.safe_total))),
  );
  return Array.from({ length: 4 }, (_, row) =>
    Array.from({ length: 8 }, (_, column) =>
      row * 8 + column < filled ? "🟨" : "🟦",
    ).join(""),
  );
}
export function shareText(card: DailyShare, summary: string): string {
  return `${summary}\n${shareMosaic(card).join("\n")}`;
}
export async function renderSharePng(
  card: DailyShare,
  summary: string,
  dom: Document = document,
): Promise<Blob> {
  const canvas = dom.createElement("canvas");
  canvas.width = 800;
  canvas.height = 600;
  const context = canvas.getContext("2d");
  if (!context) throw Error("share_unavailable");
  const color = (value: number) => `#${value.toString(16).padStart(6, "0")}`;
  context.fillStyle = color(COLORS.background);
  context.fillRect(0, 0, 800, 600);
  context.fillStyle = color(COLORS.text);
  context.font = "bold 32px sans-serif";
  context.fillText("Liar Sweeper", 48, 64);
  context.font = "20px sans-serif";
  const lines = summary.split("\n");
  lines
    .slice(0, 5)
    .forEach((line, index) =>
      context.fillText(line, 48, 110 + index * 32, 704),
    );
  context.fillText("UNVERIFIED · LOCAL", 48, 290);
  const filled = shareMosaic(card).join("").split("🟨").length - 1;
  for (let index = 0; index < 32; index++) {
    context.fillStyle = color(index < filled ? COLORS.accent : COLORS.closed);
    context.fillRect(
      48 + (index % 8) * 86,
      330 + Math.floor(index / 8) * 56,
      74,
      44,
    );
  }
  return new Promise((resolve, reject) =>
    canvas.toBlob(
      (blob) => (blob ? resolve(blob) : reject(Error("share_unavailable"))),
      "image/png",
    ),
  );
}
export function createSharePort(
  nav: Pick<Navigator, "clipboard" | "share"> = navigator,
  dom: Document = document,
): SharePort {
  return {
    async copy(text) {
      if (!nav.clipboard?.writeText) throw Error("share_unavailable");
      await nav.clipboard.writeText(text);
    },
    async nativeShare(text) {
      if (!nav.share) throw Error("share_unavailable");
      await nav.share({ title: "Liar Sweeper", text });
    },
    async download(card, text) {
      const blob = await renderSharePng(card, text, dom);
      const url = URL.createObjectURL(blob);
      try {
        const link = dom.createElement("a");
        link.href = url;
        link.download = `liar-daily-${card.date}.png`;
        link.click();
      } finally {
        setTimeout(() => URL.revokeObjectURL(url), 1000);
      }
    },
  };
}
