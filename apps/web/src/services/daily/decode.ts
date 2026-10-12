import {
  PROTOCOL_VERSION,
  type DailyCompletion,
  type DailyDefinition,
  type DailyMetadata,
  type DailyOwnResponse,
  type DailyRankingsResponse,
  type DailyStartResponse,
  type DailySubmitResponse,
  type DailyTrust,
} from "@liar/protocol";
import {
  bool,
  fail,
  int,
  list,
  one,
  optional,
  struct,
  text,
  uuid,
} from "../wire-decode";
import { decodeRulesSnapshot } from "../rules-decode";
const v = one(PROTOCOL_VERSION),
  u16 = int(65535),
  u32 = int(4294967295);
export const dateValue = (raw: unknown): string => {
  const date = text(/^\d{4}-\d{2}-\d{2}$/)(raw);
  if (new Date(date + "T00:00:00Z").toISOString().slice(0, 10) !== date) fail();
  return date;
};
export const metadataValue = struct({
  date: dateValue,
  seed_version: int(65535, 1),
  mode: one("solo-v1"),
  seed: (raw: unknown) => {
    const n = text(/^(?:0|[1-9][0-9]{0,19})$/)(raw);
    if (BigInt(n) > 18446744073709551615n) fail();
    return n;
  },
  rules_hash: text(/^[0-9a-f]{64}$/),
  solver_version: u16,
  rng_version: u16,
});
export function sameMetadata(a: DailyMetadata, b: DailyMetadata): boolean {
  return (
    a.date === b.date &&
    a.seed_version === b.seed_version &&
    a.mode === b.mode &&
    a.seed === b.seed &&
    a.rules_hash === b.rules_hash &&
    a.solver_version === b.solver_version &&
    a.rng_version === b.rng_version
  );
}
const action = (raw: unknown) => {
  const value = raw as { type?: unknown } | null;
  if (value?.type === "attack") return struct({ type: one("attack") })(raw);
  return struct({ type: one("open", "flag", "accuse"), cell: u16 })(raw);
};
export const decodeReplay = struct({
  v,
  metadata: metadataValue,
  inputs: list(
    struct({
      time_ms: u32,
      input: struct({
        v,
        command_id: int(4294967295, 1),
        client_seq: int(4294967295, 1),
        action,
      }),
    }),
    8192,
  ),
  final_time_ms: u32,
});
const definition = struct({
  v,
  metadata: metadataValue,
  rules: decodeRulesSnapshot,
});
function snapshotBinding(d: DailyDefinition): void {
  if (
    d.metadata.rules_hash !== d.rules.hash ||
    d.metadata.solver_version !== d.rules.solver_version ||
    d.metadata.rng_version !== d.rules.rng_version
  )
    fail();
}
export function decodeDefinition(
  raw: unknown,
  date: string,
  version: number,
): DailyDefinition {
  const d = definition(raw);
  snapshotBinding(d);
  if (d.metadata.date !== date || d.metadata.seed_version !== version) fail();
  return d;
}
const start = struct({
  v,
  account_id: uuid,
  attempt_id: uuid,
  ticket_id: uuid,
  metadata: metadataValue,
  rules: decodeRulesSnapshot,
  ttl_ms: int(Number.MAX_SAFE_INTEGER, 1),
  heartbeat_interval_ms: int(4294967295, 1),
});
export function decodeStart(
  raw: unknown,
  account: string,
  attempt: string,
): DailyStartResponse {
  const d = start(raw);
  snapshotBinding(d);
  if (d.account_id !== account || d.attempt_id !== attempt) fail();
  return d;
}
const heartbeat = struct({
  v,
  account_id: uuid,
  ticket_id: uuid,
  ack: optional(struct({ sequence: int(4294967295, 1), elapsed_ms: int() })),
});
export function decodeHeartbeat(
  raw: unknown,
  account: string,
  ticket: string,
  sequence: number,
) {
  const d = heartbeat(raw);
  if (
    d.account_id !== account ||
    d.ticket_id !== ticket ||
    (d.ack !== null && d.ack.sequence !== sequence)
  )
    fail();
  return d;
}
const completion = struct({
  completion_id: uuid,
  attempt_id: uuid,
  metadata: metadataValue,
  opened_safe: int(256, 1),
  mistakes: u16,
  replayed_elapsed_ms: u32,
  verified_elapsed_ms: optional(u32),
  completed_at: int(Number.MAX_SAFE_INTEGER, 1),
});
const trust = one("verified_completion", "verified_time");
function receiptBinding(
  receipt: DailyCompletion | null,
  trust: DailyTrust,
  metadata: DailyMetadata,
) {
  if (!receipt) {
    if (trust !== "verified_completion") fail();
    return;
  }
  if (
    !sameMetadata(receipt.metadata, metadata) ||
    (receipt.verified_elapsed_ms !== null) !== (trust === "verified_time") ||
    (receipt.verified_elapsed_ms !== null &&
      receipt.verified_elapsed_ms < receipt.replayed_elapsed_ms)
  )
    fail();
}
const submitted = struct({
  v,
  account_id: uuid,
  attempt_id: uuid,
  disposition: one("official", "practice"),
  trust,
  receipt: completion,
});
export function decodeSubmitted(
  raw: unknown,
  account: string,
  attempt: string,
  metadata: DailyMetadata,
): DailySubmitResponse {
  const d = submitted(raw);
  receiptBinding(d.receipt, d.trust, metadata);
  if (
    d.account_id !== account ||
    d.attempt_id !== attempt ||
    (d.disposition === "official" && d.receipt.attempt_id !== attempt)
  )
    fail();
  return d;
}
const own = struct({
  v,
  account_id: uuid,
  trust,
  receipt: optional(completion),
});
export function decodeOwn(
  raw: unknown,
  account: string,
  metadata: DailyMetadata,
): DailyOwnResponse {
  const d = own(raw);
  receiptBinding(d.receipt, d.trust, metadata);
  if (d.account_id !== account) fail();
  return d;
}
const nickname = (raw: unknown): string => {
  if (
    typeof raw !== "string" ||
    raw.length < 1 ||
    raw.length > 1024 ||
    [...raw].some((c) => c.charCodeAt(0) < 32 || c.charCodeAt(0) === 127)
  )
    fail();
  return raw;
};
const rank = { position: int(50, 1), nickname, mistakes: u16 };
const rankings = struct({
  v,
  metadata: optional(metadataValue),
  time: struct({
    entries: list(struct({ ...rank, elapsed_ms: u32 }), 50),
    has_more: bool,
  }),
  completion: struct({ entries: list(struct(rank), 50), has_more: bool }),
});
export function decodeRankings(
  raw: unknown,
  date: string,
  version: number,
): DailyRankingsResponse {
  const d = rankings(raw);
  if (
    d.metadata !== null &&
    (d.metadata.date !== date || d.metadata.seed_version !== version)
  )
    fail();
  for (const page of [d.time, d.completion]) {
    if (
      (page.has_more && page.entries.length !== 50) ||
      page.entries.some((r, i) => r.position !== i + 1) ||
      (d.metadata === null && (page.entries.length !== 0 || page.has_more))
    )
      fail();
  }
  return d;
}
export const decodeFailure = struct({
  code: one(
    "daily_invalid",
    "auth_required",
    "daily_conflict",
    "daily_expired",
    "daily_rate_limited",
    "daily_unavailable",
  ),
});
