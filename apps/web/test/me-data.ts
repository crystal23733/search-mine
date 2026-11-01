import type {
  MeOverview,
  MeHistory,
  MeOnlineRecord,
  MeDailyRecord,
} from "@liar/protocol";
import { TEST_ACCOUNT } from "./auth-ports";
export const ME_NOW = 1791331260;
export function onlineEntry(
  id = "c0000000-0000-4000-8000-000000000001",
): MeOnlineRecord {
  return {
    result: {
      match_id: id,
      rules_hash: "a".repeat(64),
      end_elapsed_ms: 240000,
      result: { reason: "timeout", outcome: "win", completed: true },
      own: {
        opened_safe: 10,
        mistakes: 1,
        accusation_attempts: 2,
        correct_accusations: 1,
      },
    },
    opponent: "human",
    recorded_at: ME_NOW,
    retention_started_at: ME_NOW,
  };
}
export function dailyEntry(): MeDailyRecord {
  return {
    completion_id: "d0000000-0000-4000-8000-000000000001",
    date: "2026-10-07",
    opened_safe: 216,
    mistakes: 0,
    replayed_elapsed_ms: 3200,
    verified_elapsed_ms: null,
    trust: "verified_completion",
    completed_at: ME_NOW,
  };
}
export function overview(): MeOverview {
  return {
    v: 1,
    account: {
      ...TEST_ACCOUNT,
      created_at: 1,
      last_seen_at: 2,
      identities: [{ provider: "google", linked_at: 1 }],
    },
    observed_at: ME_NOW,
    window_started_at: ME_NOW - 7776000,
    online: {
      wins: 1,
      losses: 0,
      draws: 0,
      aborted: 0,
      cancelled: 0,
      played: 1,
      win_rate: 1,
    },
    recent_online: [onlineEntry()],
    daily_total: 1,
    recent_daily: [dailyEntry()],
    consent: {
      cmp_available: false,
      status: "unknown",
      analytics: "unknown",
      advertising: "unknown",
      checked_at: null,
      expires_at: null,
    },
  };
}
export function history(): MeHistory {
  return {
    v: 1,
    account_id: TEST_ACCOUNT.id,
    kind: "online",
    opponent: "all",
    observed_at: ME_NOW,
    window_started_at: ME_NOW - 7776000,
    total: 1,
    online: overview().online,
    items: [{ kind: "online", entry: onlineEntry() }],
    next: null,
  };
}
