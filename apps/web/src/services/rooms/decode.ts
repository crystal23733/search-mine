import {
  ROOM_PAGE_SIZE,
  ROOM_PROTOCOL_VERSION,
  type RoomList,
  type RoomView,
  type RoomPayload,
  type RoundResult,
} from "@liar/protocol";
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

export const roomError = one(
  "malformed",
  "unsupported_version",
  "unauthorized",
  "forbidden",
  "not_found",
  "busy",
  "full",
  "capacity",
  "not_ready",
  "stale",
  "conflict",
  "rate_limited",
  "unavailable",
);
export const seat = int(3);
export const boundedName = (limit: number) => (value: unknown) =>
  typeof value === "string" &&
  value.length > 0 &&
  value.length <= limit &&
  ![...value].some(
    (letter) => letter.charCodeAt(0) < 32 || letter.charCodeAt(0) === 127,
  )
    ? value
    : fail();
const revision = (value: unknown) => {
  const parsed = text(/^[1-9][0-9]{0,19}$/)(value);
  return BigInt(parsed) <= 18446744073709551615n ? parsed : fail();
};
const phase = one("waiting", "preparing", "countdown", "playing", "results");
const capacity = int(4, 2);
const cursor = struct({ accepting: bool, room_id: uuid });
const card = struct({
  room_id: uuid,
  title: boundedName(256),
  capacity,
  members: int(4, 1),
  phase,
  accepting: bool,
});
const page = struct({
  rooms: list(card, ROOM_PAGE_SIZE),
  next: optional(cursor),
});
function roomPage(value: unknown): RoomList {
  const decoded = page(value);
  if (
    new Set(decoded.rooms.map(({ room_id }) => room_id)).size !==
      decoded.rooms.length ||
    decoded.rooms.some(
      (room) =>
        room.members > room.capacity ||
        (room.accepting && room.members === room.capacity),
    )
  )
    fail();
  return decoded;
}
const member = struct({
  seat,
  nickname: boundedName(1024),
  ready: bool,
  in_round: bool,
});
const roomFields = {
  room_id: uuid,
  revision,
  code: text(/^[ABCDEFGHJKLMNPQRSTUVWXYZ23456789]{8}$/),
  options: struct({
    title: boundedName(256),
    visibility: one("public", "invite"),
    capacity,
  }),
  phase,
  own_seat: seat,
  host_seat: seat,
  members: list(member, 4, 1),
  round: optional(struct({ round_id: uuid, seat })),
  expires_at_ms: optional(int()),
};
const room = struct(roomFields);
const result = struct({
  reason: one(
    "clear",
    "timeout",
    "forfeit",
    "abandoned",
    "server_failure",
    "cancelled",
  ),
  winner: optional(seat),
  standings: list(
    struct({
      seat,
      nickname: optional(boundedName(1024)),
      rank: optional(int(4, 1)),
      opened_safe: int(256),
      forfeited: bool,
    }),
    4,
    2,
  ),
});
export function decodeRoundResult(value: unknown): RoundResult {
  const decoded = result(value);
  if (
    new Set(decoded.standings.map((entry) => entry.seat)).size !==
      decoded.standings.length ||
    decoded.standings.some(
      (entry) =>
        entry.rank !== null &&
        (entry.rank > decoded.standings.length || entry.forfeited),
    ) ||
    (decoded.winner !== null &&
      !decoded.standings.some(
        (entry) => entry.seat === decoded.winner && entry.rank === 1,
      ))
  )
    fail();
  return decoded;
}
const resultSummary = struct({
  result: decodeRoundResult,
  recording: one("pending", "saved", "failed"),
});
function currentRoom(value: unknown): RoomView {
  const { start_error, result_summary, ...attributes } = object(value);
  const decoded = room(attributes);
  const seats = new Set(decoded.members.map((entry) => entry.seat));
  const own = decoded.members.find((entry) => entry.seat === decoded.own_seat);
  if (
    seats.size !== decoded.members.length ||
    decoded.members.length > decoded.options.capacity ||
    !own ||
    !seats.has(decoded.host_seat) ||
    own.in_round !== (decoded.round !== null)
  )
    fail();
  const summary =
    result_summary === undefined ? undefined : resultSummary(result_summary);
  if (
    summary &&
    (decoded.phase !== "results" ||
      !decoded.round ||
      !summary.result.standings.some(
        (entry) => entry.seat === decoded.round?.seat,
      ))
  )
    fail();
  return {
    ...decoded,
    ...(start_error === undefined
      ? {}
      : { start_error: roomError(start_error) }),
    ...(summary ? { result_summary: summary } : {}),
  };
}
function payload(value: unknown): RoomPayload {
  switch (object(value).type) {
    case "list":
      return struct({ type: one("list"), page: roomPage })(value);
    case "current":
      return struct({ type: one("current"), room: optional(currentRoom) })(
        value,
      );
    case "error":
      return struct({ type: one("error"), error: roomError })(value);
    default:
      return fail();
  }
}
export const decodeRoomResponse = struct({
  v: one(ROOM_PROTOCOL_VERSION),
  server_time_ms: int(),
  payload,
});
