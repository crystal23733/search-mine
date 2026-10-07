import { dailyId, type DailyRecord } from "../src/services/daily-records";
export function record(date = "2026-10-07", attempt = "first"): DailyRecord {
  const metadata = {
    date,
    seed_version: 1,
    mode: "solo-v1",
    seed: "42",
    rules_hash: "hash",
    solver_version: 1,
    rng_version: 1,
  };
  return {
    v: 1,
    id: dailyId(metadata),
    status: "unverified",
    metadata,
    attempt_id: attempt,
    elapsed_ms: 5000,
    mistakes: 0,
    replay: { v: 1, metadata, inputs: [], final_time_ms: 8000 },
  };
}
