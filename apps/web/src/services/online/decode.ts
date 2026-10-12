import { ROOM_CODE } from "./invitation";
import {
  PROTOCOL_VERSION,
  type GameView,
  type LobbyCancellation,
  type LobbyResponse,
  type LobbyState,
  type OnlineEvent,
  type OnlinePayload,
  type PersonalResult,
} from "@liar/protocol";
import {
  type Decode,
  fail,
  object,
  struct,
  int,
  bool,
  text,
  uuid,
  one,
  optional,
  list,
} from "../wire-decode";
const u32 = int(4294967295),
  u16 = int(65535);
const personalValue = struct({
  match_id: uuid,
  rules_hash: text(/^[0-9a-f]{64}$/),
  end_elapsed_ms: optional(int()),
  result: struct({
    reason: one(
      "clear",
      "timeout",
      "forfeit",
      "abandoned",
      "server_failure",
      "cancelled",
    ),
    outcome: one("win", "loss", "draw", "abort", "cancelled"),
    completed: bool,
  }),
  own: optional(
    struct({
      opened_safe: int(256),
      mistakes: int(4096),
      accusation_attempts: int(4096),
      correct_accusations: int(4096),
    }),
  ),
});
const personal = struct({ v: one(PROTOCOL_VERSION), result: personalValue });
const latestPersonal = struct({
  v: one(PROTOCOL_VERSION),
  result: optional(personalValue),
});
export function decodeLatestResult(value: unknown): PersonalResult | null {
  const { result } = latestPersonal(value);
  return result === null ? null : validatePersonal(result);
}
export function decodePersonalResult(
  value: unknown,
  matchId: string,
): PersonalResult {
  const { result } = personal(value);
  if (result.match_id !== matchId) fail();
  return validatePersonal(result);
}
function validatePersonal(result: PersonalResult): PersonalResult {
  if (
    result.own !== null &&
    result.own.correct_accusations > result.own.accusation_attempts
  )
    fail();
  if (result.own === null || result.end_elapsed_ms === null) {
    if (
      result.own !== null ||
      result.end_elapsed_ms !== null ||
      result.result.reason !== "server_failure" ||
      result.result.outcome !== "abort" ||
      result.result.completed
    )
      fail();
  }
  return result;
}
export const decodeResultFailure = struct({
  error: one(
    "malformed",
    "unsupported_version",
    "unauthorized",
    "unavailable",
    "rate_limited",
    "not_found",
  ),
});
import { decodeRulesSnapshot } from "../rules-decode";
const cell = struct({
  cell: int(255),
  state: one("closed", "safe", "mine"),
  number: optional(int(8)),
  flagged: bool,
});
const numeric = struct({
  cell: int(255),
  state: one("closed", "safe", "mine"),
  number: optional(int(8)),
});
const game = struct({
  v: one(PROTOCOL_VERSION),
  rules: decodeRulesSnapshot,
  revision: int(),
  phase: one("countdown", "playing", "finished", "aborted", "cancelled"),
  countdown_ms: u32,
  remaining_ms: u32,
  own: struct({
    cells: list(cell, 256, 1),
    gauge: u16,
    stun_ms: u32,
    history: list(list(numeric, 256), 8192),
    stats: struct({
      mistakes: u16,
      accusation_attempts: u16,
      correct_accusations: u16,
    }),
  }),
  opponent: struct({
    opened_safe: u16,
    stun_ms: u32,
    reconnect_ms: optional(u32),
  }),
  result: optional(
    struct({
      reason: one(
        "clear",
        "timeout",
        "forfeit",
        "abandoned",
        "server_failure",
        "cancelled",
      ),
      outcome: one("win", "loss", "draw", "abort", "cancelled"),
      completed: bool,
    }),
  ),
});
export function decodeView(value: unknown): GameView {
  const result = game(value);
  if (
    result.own.cells.length !==
      result.rules.rules.width * result.rules.rules.height ||
    result.own.cells.some((c, i) => c.cell !== i) ||
    (result.opponent.reconnect_ms !== null &&
      result.opponent.reconnect_ms > result.rules.rules.reconnect_grace_ms)
  )
    fail();
  return result;
}
const lobbyCode = one(
  "malformed",
  "unsupported_version",
  "unauthorized",
  "unavailable",
  "capacity",
  "rate_limited",
  "busy",
  "not_found",
  "full",
  "stale",
);
const opponent = one("human", "bot");
const seat = int(1);
const pair: Decode<[boolean, boolean]> = (v) => {
  const a = list(bool, 2, 2)(v);
  return [a[0], a[1]];
};
function cancellation(v: unknown): LobbyCancellation {
  const o = object(v);
  if (o.kind === "queue")
    return struct({ kind: one("queue"), queue_id: uuid })(v);
  if (o.kind === "room") return struct({ kind: one("room"), room_id: uuid })(v);
  if (o.kind === "preparing")
    return struct({
      kind: one("preparing"),
      entity: uuid,
      generation: (v: unknown) => {
        const s = text(/^[1-9][0-9]{0,19}$/)(v);
        if (BigInt(s) > 18446744073709551615n) fail();
        return s;
      },
    })(v);
  return fail();
}
function state(v: unknown): LobbyState {
  switch (object(v).type) {
    case "idle":
      return struct({ type: one("idle") })(v);
    case "queued":
      return struct({
        type: one("queued"),
        queue_id: uuid,
        deadline_ms: int(),
        difficulty: one("easy", "normal", "hard"),
      })(v);
    case "room":
      return struct({
        type: one("room"),
        room_id: uuid,
        code: text(ROOM_CODE),
        own_seat: seat,
        occupied: pair,
        ready: pair,
        expires_at_ms: int(),
      })(v);
    case "preparing":
      return struct({
        type: one("preparing"),
        identity: cancellation,
        opponent,
      })(v);
    case "matched":
      return struct({
        type: one("matched"),
        match_id: uuid,
        own_seat: seat,
        opponent,
      })(v);
    case "failed":
      return struct({ type: one("failed"), error: lobbyCode })(v);
    default:
      return fail();
  }
}
export const decodeLobby: Decode<LobbyResponse> = struct({
  v: one(PROTOCOL_VERSION),
  server_time_ms: int(),
  state,
});
export const decodeLobbyFailure = struct({ error: lobbyCode });
const publicError = one(
  "malformed",
  "unsupported_version",
  "invalid_cell",
  "not_started",
  "stunned",
  "unavailable",
  "finished",
  "flagged",
  "already_open",
  "stale",
  "command_limit",
  "conflict",
  "invalid_time",
  "disconnected",
  "invalid_epoch",
  "invalid_sequence",
);
function payload(v: unknown): OnlinePayload {
  switch (object(v).type) {
    case "snapshot":
      return struct({
        type: one("snapshot"),
        session_epoch: int(4294967295, 1),
        last_client_seq: u32,
        view: decodeView,
      })(v);
    case "delta":
      return struct({ type: one("delta"), view: decodeView })(v);
    case "ack":
      return struct({
        type: one("ack"),
        command_id: uuid,
        revision: u32,
        status: one("applied", "rejected"),
        error: optional(publicError),
        duplicate: bool,
      })(v);
    case "error":
      return struct({
        type: one("error"),
        code: one(
          "malformed",
          "unauthorized",
          "unavailable",
          "rate_limited",
          "capacity",
          "not_matched",
          "wrong_match",
          "invalid_epoch",
          "unsupported_version",
          "invalid_cell",
        ),
      })(v);
    case "match_end":
      return struct({
        type: one("match_end"),
        view: decodeView,
        recording: one("pending", "saved", "failed"),
      })(v);
    default:
      return fail();
  }
}
export const decodeEvent: Decode<OnlineEvent> = struct({
  v: one(PROTOCOL_VERSION),
  match_id: uuid,
  server_seq: int(4294967295, 1),
  server_time_ms: int(),
  payload,
});
