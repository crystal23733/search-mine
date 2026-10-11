import {
  DEFAULT_RULES,
  DAILY_SEED_VERSION,
  PROTOCOL_VERSION,
  type DailyMetadata,
  type DailyReplay,
  type ExportRecord,
  type ExportStream,
  type GameExportInput,
  type GameExportPage,
  type PersonalDataExport,
  type ReplayExport,
} from "@liar/protocol";
import { object, decodeExport } from "./decode";
import { decodePersonalResult } from "../online/decode";
import { AuthError, type AuthTransport } from "./types";
const invalid = (): never => {
  throw new AuthError("auth_unavailable");
};
const bytes = (value: unknown) =>
  new TextEncoder().encode(JSON.stringify(value)).length;
export const EXPORT_MAX_BYTES = 32 * 1024 * 1024;
async function pause(signal: AbortSignal, milliseconds: number): Promise<void> {
  if (signal.aborted) return invalid();
  await new Promise<void>((resolve, reject) => {
    const abort = () => {
      clearTimeout(timer);
      signal.removeEventListener("abort", abort);
      reject(new AuthError("auth_invalid"));
    };
    const timer = setTimeout(() => {
      signal.removeEventListener("abort", abort);
      resolve();
    }, milliseconds);
    signal.addEventListener("abort", abort, { once: true });
  });
}
export function exportId(value: unknown): string {
  if (
    typeof value !== "string" ||
    !/^[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$/.test(value) ||
    value === "00000000-0000-0000-0000-000000000000"
  )
    return invalid();
  return value;
}
function uint(value: unknown, max = 0xffffffff): number {
  if (
    typeof value !== "number" ||
    !Number.isSafeInteger(value) ||
    value < 0 ||
    value > max
  )
    return invalid();
  return value;
}
function timestamp(value: unknown): number {
  return uint(value, 253402300799);
}
function metadata(value: unknown): DailyMetadata {
  const v = object(value, [
    "date",
    "seed_version",
    "mode",
    "seed",
    "rules_hash",
    "solver_version",
    "rng_version",
  ]);
  if (
    typeof v.date !== "string" ||
    !/^\d{4}-\d{2}-\d{2}$/.test(v.date) ||
    v.date.startsWith("0000-") ||
    new Date(`${v.date}T00:00:00Z`).toISOString().slice(0, 10) !== v.date ||
    v.seed_version !== DAILY_SEED_VERSION ||
    v.mode !== "solo-v1" ||
    typeof v.seed !== "string" ||
    !/^(0|[1-9]\d{0,19})$/.test(v.seed) ||
    BigInt(v.seed) > 0xffffffffffffffffn ||
    typeof v.rules_hash !== "string" ||
    !/^[0-9a-f]{64}$/.test(v.rules_hash)
  )
    return invalid();
  return {
    date: v.date,
    seed_version: DAILY_SEED_VERSION,
    mode: "solo-v1",
    seed: v.seed,
    rules_hash: v.rules_hash,
    solver_version: uint(v.solver_version, 65535),
    rng_version: uint(v.rng_version, 65535),
  };
}
function sameMetadata(a: DailyMetadata, b: DailyMetadata): boolean {
  return Object.keys(a).every(
    (key) => a[key as keyof DailyMetadata] === b[key as keyof DailyMetadata],
  );
}
function replay(value: unknown): DailyReplay {
  const v = object(value, ["v", "metadata", "inputs", "final_time_ms"]);
  if (
    v.v !== PROTOCOL_VERSION ||
    !Array.isArray(v.inputs) ||
    v.inputs.length > DEFAULT_RULES.rules.max_commands_per_seat ||
    bytes(value) > 2 * 1024 * 1024
  )
    return invalid();
  const inputs = v.inputs.map((raw) => {
    const entry = object(raw, ["input", "time_ms"]),
      input = object(entry.input, ["v", "command_id", "client_seq", "action"]);
    if (
      input.v !== PROTOCOL_VERSION ||
      uint(input.command_id) === 0 ||
      uint(input.client_seq) === 0
    )
      return invalid();
    const rawAction = input.action as { type?: unknown };
    if (!rawAction || typeof rawAction !== "object") return invalid();
    const action =
      rawAction.type === "attack"
        ? object(rawAction, ["type"])
        : object(rawAction, ["type", "cell"]);
    if (
      action.type !== "attack" &&
      !["open", "flag", "accuse"].includes(action.type as string)
    )
      return invalid();
    return {
      time_ms: uint(entry.time_ms),
      input: {
        v: PROTOCOL_VERSION,
        command_id: uint(input.command_id),
        client_seq: uint(input.client_seq),
        action:
          action.type === "attack"
            ? { type: "attack" as const }
            : {
                type: action.type as "open" | "flag" | "accuse",
                cell: uint(action.cell, 65535),
              },
      },
    };
  });
  return {
    v: PROTOCOL_VERSION,
    metadata: metadata(v.metadata),
    inputs,
    final_time_ms: uint(v.final_time_ms),
  };
}
export function decodeGameRecord(value: unknown): ExportRecord {
  const kind = (value as { kind?: unknown })?.kind;
  if (kind === "online") {
    const v = object(value, [
      "kind",
      "result",
      "recorded_at",
      "retention_started_at",
    ]);
    const matchId = exportId((v.result as { match_id?: unknown })?.match_id);
    const result = decodePersonalResult(
      { v: PROTOCOL_VERSION, result: v.result },
      matchId,
    );
    const recorded_at = timestamp(v.recorded_at),
      retention_started_at = timestamp(v.retention_started_at);
    if (retention_started_at > recorded_at) return invalid();
    return { kind, result, recorded_at, retention_started_at };
  }
  if (kind !== "daily") return invalid();
  const v = object(value, [
    "kind",
    "completion_id",
    "attempt_id",
    "metadata",
    "opened_safe",
    "mistakes",
    "replayed_elapsed_ms",
    "verified_elapsed_ms",
    "completed_at",
    "replay_available",
  ]);
  if (
    typeof v.replay_available !== "boolean" ||
    uint(v.opened_safe, 65535) === 0 ||
    timestamp(v.completed_at) === 0
  )
    return invalid();
  const replayedElapsed = uint(v.replayed_elapsed_ms);
  const verifiedElapsed =
    v.verified_elapsed_ms === null ? null : uint(v.verified_elapsed_ms);
  if (verifiedElapsed !== null && verifiedElapsed < replayedElapsed)
    return invalid();
  return {
    kind,
    completion_id: exportId(v.completion_id),
    attempt_id: exportId(v.attempt_id),
    metadata: metadata(v.metadata),
    opened_safe: uint(v.opened_safe, 65535),
    mistakes: uint(v.mistakes, 65535),
    replayed_elapsed_ms: replayedElapsed,
    verified_elapsed_ms: verifiedElapsed,
    completed_at: timestamp(v.completed_at),
    replay_available: v.replay_available,
  };
}
function recordId(record: ExportRecord) {
  return record.kind === "online"
    ? record.result.match_id
    : record.completion_id;
}
export function decodeGamePage(value: unknown): GameExportPage {
  const v = object(value, ["v", "account_id", "stream", "records", "next"]);
  if (
    v.v !== PROTOCOL_VERSION ||
    !["online", "daily"].includes(v.stream as string) ||
    !Array.isArray(v.records) ||
    v.records.length > 50 ||
    bytes(value) > 65536
  )
    return invalid();
  const records = v.records.map(decodeGameRecord);
  let previous = "";
  for (const record of records) {
    const current = recordId(record);
    if (record.kind !== v.stream || current <= previous) return invalid();
    previous = current;
  }
  const next = v.next === null ? null : exportId(v.next);
  if (next !== null && (records.length !== 50 || next !== previous))
    return invalid();
  return {
    v: PROTOCOL_VERSION,
    account_id: exportId(v.account_id),
    stream: v.stream as ExportStream,
    records,
    next,
  };
}
export function decodeReplayExport(value: unknown): ReplayExport {
  const v = object(value, ["v", "account_id", "completion_id", "replay"]);
  if (v.v !== PROTOCOL_VERSION) return invalid();
  return {
    v: PROTOCOL_VERSION,
    account_id: exportId(v.account_id),
    completion_id: exportId(v.completion_id),
    replay: v.replay === null ? null : replay(v.replay),
  };
}
export function decodePersonalDataExport(value: unknown): PersonalDataExport {
  const v = object(value, ["v", "account", "games", "replays"]);
  if (
    v.v !== PROTOCOL_VERSION ||
    !Array.isArray(v.games) ||
    !Array.isArray(v.replays) ||
    v.games.length > 2048 * 50 ||
    v.replays.length > v.games.length ||
    bytes(value) > EXPORT_MAX_BYTES
  )
    return invalid();
  const account = decodeExport(v.account),
    games = v.games.map(decodeGameRecord),
    replays = v.replays.map(decodeReplayExport);
  const daily = new Map(
    games.filter((g) => g.kind === "daily").map((g) => [g.completion_id, g]),
  );
  if (
    new Set(games.map((g) => `${g.kind}/${recordId(g)}`)).size !==
      games.length ||
    new Set(replays.map((r) => r.completion_id)).size !== replays.length ||
    replays.length !==
      [...daily.values()].filter((d) => d.replay_available).length
  )
    return invalid();
  for (const item of replays) {
    const saved = daily.get(item.completion_id);
    if (
      item.account_id !== account.account.id ||
      !saved?.replay_available ||
      (item.replay && !sameMetadata(item.replay.metadata, saved.metadata))
    )
      return invalid();
  }
  return { v: PROTOCOL_VERSION, account, games, replays };
}
export async function collectGameExport(
  transport: AuthTransport,
  account: PersonalDataExport["account"],
  revision: string,
  csrf: string,
  signal: AbortSignal,
): Promise<PersonalDataExport> {
  const games: ExportRecord[] = [],
    replays: ReplayExport[] = [];
  let pages = 0,
    size = bytes(account);
  let sentAt: number | undefined;
  const pace = async () => {
    if (signal.aborted) return invalid();
    if (sentAt !== undefined)
      await pause(signal, Math.max(0, 100 - (performance.now() - sentAt)));
    if (signal.aborted) return invalid();
    sentAt = performance.now();
  };
  const accept = (value: unknown) => {
    if (signal.aborted || (size += bytes(value)) > EXPORT_MAX_BYTES)
      return invalid();
  };
  for (const stream of ["online", "daily"] as const) {
    let after: string | null = null;
    for (;;) {
      if (++pages > 2048 || signal.aborted) return invalid();
      const input: GameExportInput = {
        v: PROTOCOL_VERSION,
        account_id: account.account.id,
        session_revision: revision,
        stream,
        after,
      };
      await pace();
      const page = decodeGamePage(
        await transport.gameExport(input, csrf, signal),
      );
      accept(page);
      if (
        page.account_id !== account.account.id ||
        page.stream !== stream ||
        (after !== null && page.records.some((r) => recordId(r) <= after!))
      )
        return invalid();
      games.push(...page.records);
      if (page.next === null) break;
      if (after !== null && page.next <= after) return invalid();
      after = page.next;
    }
  }
  for (const saved of games) {
    if (saved.kind !== "daily" || !saved.replay_available) continue;
    if (signal.aborted) return invalid();
    await pace();
    const result = decodeReplayExport(
      await transport.replayExport(
        {
          v: PROTOCOL_VERSION,
          account_id: account.account.id,
          session_revision: revision,
          completion_id: saved.completion_id,
        },
        csrf,
        signal,
      ),
    );
    accept(result);
    if (
      result.account_id !== account.account.id ||
      result.completion_id !== saved.completion_id ||
      (result.replay && !sameMetadata(result.replay.metadata, saved.metadata))
    )
      return invalid();
    replays.push(result);
  }
  return decodePersonalDataExport({
    v: PROTOCOL_VERSION,
    account,
    games,
    replays,
  });
}
