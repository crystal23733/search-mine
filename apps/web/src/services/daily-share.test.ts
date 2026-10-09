// @vitest-environment node
import { expect, test } from "vitest";
import { DEFAULT_RULES, type DailyView } from "@liar/protocol";
import { dailyShare, shareMosaic, shareText } from "./daily-share";
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
