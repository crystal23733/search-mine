import { DEFAULT_RULES, type GameView } from "@liar/protocol";
export const matchId = "11111111-1111-4111-8111-111111111111";
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
    opponent: { opened_safe: 0, stun_ms: 0 },
    result: null,
  };
}
