import {
  DEFAULT_RULES,
  type GameView,
  type PersonalResult,
} from "@liar/protocol";
export const matchId = "11111111-1111-4111-8111-111111111111";
export function personalResult(): PersonalResult {
  return {
    match_id: matchId,
    rules_hash: "a".repeat(64),
    end_elapsed_ms: 240000,
    result: { reason: "timeout", outcome: "win", completed: true },
    own: {
      opened_safe: 8,
      mistakes: 2,
      accusation_attempts: 3,
      correct_accusations: 1,
    },
  };
}
export function view(): GameView {
  return {
    v: 1,
    rules: structuredClone(DEFAULT_RULES),
    revision: 0,
    phase: "countdown",
    countdown_ms: 3000,
    remaining_ms: 240000,
    own: {
      cells: Array.from({ length: 256 }, (_, cell) => ({
        cell,
        state: "closed",
        number: null,
        flagged: false,
      })),
      gauge: 0,
      stun_ms: 0,
      history: [],
      stats: { mistakes: 0, accusation_attempts: 0, correct_accusations: 0 },
    },
    opponent: { opened_safe: 0, stun_ms: 0, reconnect_ms: null },
    result: null,
  };
}
