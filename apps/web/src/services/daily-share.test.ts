// @vitest-environment node
import { expect, test, vi } from "vitest";
import {
  DEFAULT_RULES,
  type DailyView,
  type DailyCompletion,
} from "@liar/protocol";
import {
  dailyShare,
  verifiedDailyShare,
  shareMosaic,
  shareText,
  createSharePort,
} from "./daily-share";
test("web summaries only copy or share text and never expose a file download operation", async () => {
  const copy = vi.fn(async () => {}),
    share = vi.fn(async () => {});
  const port = createSharePort({
    clipboard: { writeText: copy } as unknown as Clipboard,
    share,
  });
  expect("download" in port).toBe(false);
  await port.copy("visible result");
  await port.nativeShare("visible result");
  expect(copy).toHaveBeenCalledWith("visible result");
  expect(share).toHaveBeenCalledWith({
    title: "Liar Sweeper",
    text: "visible result",
  });
});
test("share allowlist ignores names, seed, replay and actual board positions and always states local trust", () => {
  const view = {
    metadata: {
      date: "2026-10-07",
      seed_version: 1,
      seed: "private-card-seed",
    },
    rules: DEFAULT_RULES,
    completed: false,
    elapsed_ms: 5000,
    own: {
      cells: [
        { state: "safe", number: 3, cell: 200 },
        { state: "mine", cell: 15 },
      ],
      stats: { mistakes: 1 },
    },
    nickname: "DO-NOT-SHARE",
    replay: "DO-NOT-SHARE",
  } as unknown as DailyView;
  const card = dailyShare(view);
  expect(card).toEqual({
    date: "2026-10-07",
    version: 1,
    completed: false,
    opened_safe: 1,
    safe_total: 216,
    elapsed_ms: 5000,
    mistakes: 1,
    trust: "unverified",
  });
  expect(JSON.stringify(card)).not.toMatch(
    /seed|nickname|number|replay|cell|DO-NOT/,
  );
  const altered = {
    ...view,
    own: {
      ...view.own,
      cells: [{ ...view.own.cells[0], cell: 0, number: 8 }, view.own.cells[1]],
    },
  };
  expect(shareMosaic(dailyShare(altered))).toEqual(shareMosaic(card));
  expect(shareMosaic(card).join("")).toHaveLength(64);
  const complete = { ...card, completed: true, opened_safe: 216 };
  expect(shareMosaic(complete).join("")).toBe("🟨".repeat(32));
  expect(shareText(complete, "Liar Sweeper · local unverified")).toContain(
    "unverified",
  );
});
test("verified summaries select the receipt authority without private fields", async () => {
  const receipt = {
    completion_id: "PRIVATE-COMPLETION",
    attempt_id: "PRIVATE-ATTEMPT",
    metadata: { date: "2026-10-07", seed_version: 1, seed: "PRIVATE-SEED" },
    opened_safe: 216,
    mistakes: 3,
    replayed_elapsed_ms: 9000,
    verified_elapsed_ms: 12345,
  } as DailyCompletion;
  const timed = verifiedDailyShare(receipt, "verified_time", DEFAULT_RULES);
  const untimed = verifiedDailyShare(
    { ...receipt, verified_elapsed_ms: null },
    "verified_completion",
    DEFAULT_RULES,
  );
  expect(timed.elapsed_ms).toBe(12345);
  expect(untimed.elapsed_ms).toBe(9000);
  expect(JSON.stringify([timed, untimed])).not.toMatch(
    /PRIVATE|seed|attempt|completion_id/,
  );
  expect(() =>
    verifiedDailyShare(
      { ...receipt, verified_elapsed_ms: null },
      "verified_time",
      DEFAULT_RULES,
    ),
  ).toThrow("verified_time_missing");
});
