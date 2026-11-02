import type { MeOverview, MeHistory, MeRecord } from "@liar/protocol";
import {
  bool,
  fail,
  int,
  list,
  object,
  one,
  optional,
  struct,
  text,
  uuid,
} from "../wire-decode";
import { decodePersonalResult } from "../online/decode";
import { dateValue } from "../daily/decode";
import type { MeSelection } from "./types";
const time = int(253402300799),
  count = int(10000),
  kind = one("online", "daily"),
  opponent = one("all", "human", "bot");
const identity = struct({
  provider: one("google", "apple", "kakao", "naver"),
  linked_at: time,
});
const nickname = (raw: unknown): string | null => {
  if (raw === null) return null;
  if (
    typeof raw !== "string" ||
    raw.length === 0 ||
    raw.length > 1024 ||
    [...raw].some((c) => c.charCodeAt(0) < 32 || c.charCodeAt(0) === 127)
  )
    fail();
  return raw;
};
const account = struct({
  id: uuid,
  nickname,
  created_at: time,
  last_seen_at: time,
  identities: list(identity, 4),
});
const rate = (value: unknown): number =>
  typeof value === "number" &&
  Number.isFinite(value) &&
  value >= 0 &&
  value <= 1
    ? value
    : fail();
const summary = struct({
  wins: count,
  losses: count,
  draws: count,
  aborted: count,
  cancelled: count,
  played: count,
  win_rate: optional(rate),
});
const choice = one("unknown", "denied", "granted");
const consent = struct({
  cmp_available: bool,
  status: one("unknown", "current", "expired"),
  analytics: choice,
  advertising: choice,
  checked_at: optional(time),
  expires_at: optional(time),
});
const personal = (value: unknown) =>
  decodePersonalResult({ v: 1, result: value }, uuid(object(value).match_id));
const online = struct({
  result: personal,
  opponent: one("human", "bot", "unknown"),
  retention_started_at: time,
  recorded_at: time,
});
const daily = struct({
  completion_id: uuid,
  date: dateValue,
  opened_safe: int(4294967295),
  mistakes: int(65535),
  replayed_elapsed_ms: int(4294967295),
  verified_elapsed_ms: optional(int(4294967295)),
  trust: one("verified_completion", "verified_time"),
  completed_at: time,
});
const record = (value: unknown): MeRecord =>
  object(value).kind === "online"
    ? struct({ kind: one("online"), entry: online })(value)
    : struct({ kind: one("daily"), entry: daily })(value);
const overview = struct({
  v: one(1),
  account,
  observed_at: time,
  window_started_at: time,
  online: summary,
  recent_online: list(online, 3),
  daily_total: count,
  recent_daily: list(daily, 3),
  consent,
});
const history = struct({
  v: one(1),
  account_id: uuid,
  kind,
  opponent,
  observed_at: time,
  window_started_at: time,
  total: count,
  online: optional(summary),
  items: list(record, 20),
  next: optional(text(/^[A-Za-z0-9_-]{1,512}$/)),
});
export const decodeMeFailure = struct({
  error: one(
    "malformed",
    "unsupported_version",
    "unauthorized",
    "unavailable",
    "rate_limited",
  ),
});
function period(from: number, to: number) {
  if (from > to) fail();
}
function onlinePeriod(
  rows: ReturnType<typeof online>[],
  from: number,
  to: number,
) {
  const ids = new Set<string>();
  for (const row of rows) {
    if (
      row.retention_started_at > row.recorded_at ||
      row.retention_started_at < from ||
      row.recorded_at > to ||
      ids.has(row.result.match_id)
    )
      fail();
    ids.add(row.result.match_id);
  }
}
function dailyPeriod(
  rows: ReturnType<typeof daily>[],
  from: number,
  to: number,
) {
  const ids = new Set<string>();
  for (const row of rows) {
    if (
      row.completed_at < from ||
      row.completed_at > to ||
      ids.has(row.completion_id) ||
      (row.trust === "verified_time") !== (row.verified_elapsed_ms !== null)
    )
      fail();
    ids.add(row.completion_id);
  }
}
export function decodeMeOverview(raw: unknown, owner: string): MeOverview {
  const value = overview(raw);
  period(value.window_started_at, value.observed_at);
  const account = value.account;
  if (
    account.id !== owner ||
    account.created_at > account.last_seen_at ||
    account.last_seen_at > value.observed_at ||
    new Set(account.identities.map((i) => i.provider)).size !==
      account.identities.length ||
    account.identities.some((i) => i.linked_at > value.observed_at)
  )
    fail();
  onlinePeriod(value.recent_online, value.window_started_at, value.observed_at);
  dailyPeriod(value.recent_daily, value.window_started_at, value.observed_at);
  const c = value.consent;
  if (c.status === "unknown") {
    if (
      c.checked_at !== null ||
      c.expires_at !== null ||
      c.analytics !== "unknown" ||
      c.advertising !== "unknown"
    )
      fail();
  } else {
    if (
      c.checked_at === null ||
      c.expires_at === null ||
      c.checked_at > value.observed_at ||
      c.expires_at <= c.checked_at
    )
      fail();
    if (
      c.status === "current" &&
      (c.expires_at <= value.observed_at ||
        c.analytics === "unknown" ||
        c.advertising === "unknown")
    )
      fail();
    if (
      c.status === "expired" &&
      (c.expires_at > value.observed_at ||
        c.analytics !== "unknown" ||
        c.advertising !== "unknown")
    )
      fail();
  }
  return value;
}
export function decodeMeHistory(
  raw: unknown,
  owner: string,
  selection: MeSelection,
): MeHistory {
  const value = history(raw);
  period(value.window_started_at, value.observed_at);
  if (
    value.account_id !== owner ||
    value.kind !== selection.kind ||
    value.opponent !== selection.opponent ||
    value.items.length > value.total ||
    value.items.some((i) => i.kind !== value.kind)
  )
    fail();
  if (value.kind === "daily") {
    if (value.opponent !== "all" || value.online !== null) fail();
    dailyPeriod(
      value.items.filter((i) => i.kind === "daily").map((i) => i.entry),
      value.window_started_at,
      value.observed_at,
    );
  } else {
    if (value.online === null) fail();
    const rows = value.items
      .filter((i) => i.kind === "online")
      .map((i) => i.entry);
    if (
      value.opponent !== "all" &&
      rows.some((i) => i.opponent !== value.opponent)
    )
      fail();
    onlinePeriod(rows, value.window_started_at, value.observed_at);
  }
  return value;
}
