import { expect, test } from "vitest";
import { DEFAULT_RULES, type GameView } from "@liar/protocol";
import { decodeView, decodeLobby, decodeEvent } from "./decode";
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
test("projects valid public snapshots and rejects secrets at every nested boundary", () => {
  const value = view();
  expect(decodeView(value)).toEqual(value);
  expect(decodeView(value)).not.toBe(value);
  for (const altered of [
    { ...value, seed: "secret" },
    { ...value, own: { ...value.own, truth: [] } },
    { ...value, rules: { ...value.rules, credential: "secret" } },
    {
      ...value,
      own: {
        ...value.own,
        cells: [
          { ...value.own.cells[0], actual: 4 },
          ...value.own.cells.slice(1),
        ],
      },
    },
  ])
    expect(() => decodeView(altered)).toThrow();
});
test("rejects unsupported wire versions, unsafe integers, malformed cell arrays and unknown variants", () => {
  const value = view();
  for (const altered of [
    { ...value, v: 2 },
    { ...value, revision: Number.MAX_SAFE_INTEGER + 1 },
    { ...value, phase: "won" },
    { ...value, own: { ...value.own, cells: value.own.cells.slice(1) } },
    {
      ...value,
      own: {
        ...value.own,
        cells: [
          { ...value.own.cells[0], cell: 256 },
          ...value.own.cells.slice(1),
        ],
      },
    },
  ])
    expect(() => decodeView(altered)).toThrow();
  expect(decodeView(value)).toEqual(value);
});
test("decodes queue, exact preparing identity and matched assignment without any account data", () => {
  for (const state of [
    { type: "idle" },
    {
      type: "queued",
      queue_id: matchId,
      deadline_ms: 10000,
      difficulty: "hard",
    },
    {
      type: "preparing",
      identity: {
        kind: "preparing",
        entity: matchId,
        generation: "18446744073709551615",
      },
      opponent: "bot",
    },
    { type: "matched", match_id: matchId, own_seat: 1, opponent: "human" },
  ]) {
    const value = { v: 1, server_time_ms: 0, state };
    expect(decodeLobby(value)).toEqual(value);
    expect(() =>
      decodeLobby({ ...value, state: { ...state, account_id: matchId } }),
    ).toThrow();
  }
  expect(() =>
    decodeLobby({
      v: 1,
      server_time_ms: 0,
      state: {
        type: "preparing",
        identity: {
          kind: "preparing",
          entity: matchId,
          generation: "18446744073709551616",
        },
        opponent: "bot",
      },
    }),
  ).toThrow();
});
test("accepts only a required own u32 command cursor in the snapshot", () => {
  const value = {
    v: 1,
    match_id: matchId,
    server_seq: 9,
    server_time_ms: 1000,
    payload: {
      type: "snapshot",
      session_epoch: 3,
      last_client_seq: 0,
      view: view(),
    },
  };
  for (const cursor of [0, 41, 4294967295])
    expect(
      decodeEvent({
        ...value,
        payload: { ...value.payload, last_client_seq: cursor },
      }).payload,
    ).toMatchObject({ last_client_seq: cursor });
  for (const cursor of [undefined, null, -1, 1.2, 4294967296, "41"])
    expect(() =>
      decodeEvent({
        ...value,
        payload: { ...value.payload, last_client_seq: cursor },
      }),
    ).toThrow();
  expect(() =>
    decodeEvent({
      ...value,
      payload: { ...value.payload, opponent_last_client_seq: 4 },
    }),
  ).toThrow();
});

test("decodes all event variants and rejects foreign envelope fields or malformed ACKs", () => {
  for (const payload of [
    { type: "snapshot", last_client_seq: 0, session_epoch: 1, view: view() },
    { type: "delta", view: view() },
    {
      type: "ack",
      command_id: matchId,
      revision: 0,
      status: "rejected",
      error: "not_started",
      duplicate: false,
    },
    { type: "error", code: "unauthorized" },
    {
      type: "match_end",
      view: {
        ...view(),
        phase: "finished",
        result: { reason: "timeout", outcome: "draw", completed: true },
      },
      recording: "pending",
    },
  ]) {
    const value = {
      v: 1,
      match_id: matchId,
      server_seq: 1,
      server_time_ms: 0,
      payload,
    };
    expect(decodeEvent(value)).toEqual(value);
    expect(() => decodeEvent({ ...value, account: matchId })).toThrow();
  }
  expect(() =>
    decodeEvent({
      v: 1,
      match_id: matchId,
      server_seq: 1,
      server_time_ms: 0,
      payload: {
        type: "ack",
        command_id: "bad",
        revision: 0,
        status: "ok",
        error: null,
        duplicate: false,
      },
    }),
  ).toThrow();
});
