import { DEFAULT_RULES, type DailyView } from "@liar/protocol";
export function officialView(): DailyView {
  return {
    v: 1,
    rules: DEFAULT_RULES,
    metadata: {
      date: "2026-10-07",
      seed_version: 1,
      mode: "solo-v1",
      seed: "42",
      rules_hash: DEFAULT_RULES.hash,
      solver_version: 1,
      rng_version: 1,
    },
    own: {
      cells: Array.from({ length: 256 }, (_, cell) => ({
        cell,
        state: cell === 0 ? "safe" : "closed",
        number: cell === 0 ? 0 : null,
        flagged: false,
      })),
      gauge: 20,
      stun_ms: 0,
      history: [],
      stats: { mistakes: 0, accusation_attempts: 0, correct_accusations: 0 },
    },
    revision: 0,
    phase: "playing",
    countdown_ms: 0,
    remaining_ms: 240000,
    completed: false,
    elapsed_ms: 0,
    available_actions: ["open", "flag"],
  };
}
export function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
