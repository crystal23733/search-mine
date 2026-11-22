import {
  ROOM_PROTOCOL_VERSION,
  type RoundEvent,
  type RoundPayload,
  type RoundView,
} from "@liar/protocol";
import { decodeOpponentProgress, decodeOwnBoard } from "../online/decode";
import { decodeRulesSnapshot } from "../rules-decode";
import {
  bool,
  fail,
  int,
  list,
  object,
  one,
  optional,
  struct,
  uuid,
} from "../wire-decode";
import { boundedName, decodeRoundResult, seat } from "./decode";

const u32 = int(4294967295);
const endpointCode = one(
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
);
const commandCode = one(
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
const roundView = struct({
  rules: decodeRulesSnapshot,
  own: decodeOwnBoard,
  forfeited: bool,
  opponents: list(
    struct({
      seat,
      nickname: optional(boundedName(1024)),
      progress: decodeOpponentProgress,
      forfeited: bool,
    }),
    3,
    1,
  ),
  revision: int(),
  phase: one("countdown", "playing", "finished", "aborted", "cancelled"),
  countdown_ms: u32,
  remaining_ms: u32,
  result: optional(decodeRoundResult),
});
function view(raw: unknown): RoundView {
  const value = roundView(raw);
  const { rules } = value.rules;
  if (
    value.own.cells.length !== rules.width * rules.height ||
    value.own.cells.some((cell, index) => cell.cell !== index) ||
    new Set(value.opponents.map((entry) => entry.seat)).size !==
      value.opponents.length ||
    value.opponents.some(
      (entry) =>
        entry.progress.opened_safe > rules.width * rules.height - rules.mines ||
        (entry.progress.reconnect_ms !== null &&
          entry.progress.reconnect_ms > rules.reconnect_grace_ms),
    )
  )
    fail();
  if (
    value.result &&
    value.result.standings.length !== value.opponents.length + 1
  )
    fail();
  return value;
}
function payload(raw: unknown): RoundPayload {
  switch (object(raw).type) {
    case "awaiting": {
      const event = struct({
        type: one("awaiting"),
        own_seat: seat,
        connected: int(4, 1),
        participant_count: int(4, 2),
      })(raw);
      if (
        event.connected > event.participant_count ||
        event.own_seat >= event.participant_count
      )
        fail();
      return event;
    }
    case "snapshot": {
      const event = struct({
        type: one("snapshot"),
        own_seat: seat,
        session_epoch: int(4294967295, 1),
        last_client_seq: u32,
        view,
      })(raw);
      if (event.view.opponents.some((entry) => entry.seat === event.own_seat))
        fail();
      return event;
    }
    case "delta":
      return struct({ type: one("delta"), view })(raw);
    case "end":
      return struct({
        type: one("end"),
        view,
        recording: one("pending", "saved", "failed"),
      })(raw);
    case "ack":
      return struct({
        type: one("ack"),
        command_id: uuid,
        revision: int(),
        status: one("applied", "rejected"),
        error: optional(commandCode),
        duplicate: bool,
      })(raw);
    case "error":
      return struct({ type: one("error"), code: endpointCode })(raw);
    default:
      return fail();
  }
}
export const decodeRoundEvent: (raw: unknown) => RoundEvent = struct({
  v: one(ROOM_PROTOCOL_VERSION),
  room_id: uuid,
  round_id: uuid,
  server_seq: int(4294967295, 1),
  server_time_ms: int(),
  payload,
});
