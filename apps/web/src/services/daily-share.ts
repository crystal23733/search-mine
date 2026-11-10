import type {
  DailyCompletion,
  DailyTrust,
  DailyView,
  RulesSnapshot,
} from "@liar/protocol";
export interface DailyShare {
  date: string;
  version: number;
  completed: boolean;
  opened_safe: number;
  safe_total: number;
  elapsed_ms: number;
  mistakes: number;
  trust: "unverified" | DailyTrust;
}
export interface SharePort {
  copy(text: string): Promise<void>;
  nativeShare(text: string): Promise<void>;
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
export function verifiedDailyShare(
  receipt: DailyCompletion,
  trust: DailyTrust,
  rules: RulesSnapshot,
): DailyShare {
  if (trust === "verified_time" && receipt.verified_elapsed_ms === null)
    throw Error("verified_time_missing");
  return {
    date: receipt.metadata.date,
    version: receipt.metadata.seed_version,
    completed: true,
    opened_safe: receipt.opened_safe,
    safe_total: rules.rules.width * rules.rules.height - rules.rules.mines,
    elapsed_ms:
      trust === "verified_time"
        ? receipt.verified_elapsed_ms!
        : receipt.replayed_elapsed_ms,
    mistakes: receipt.mistakes,
    trust,
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
export function createSharePort(
  nav: Pick<Navigator, "clipboard" | "share"> = navigator,
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
  };
}
