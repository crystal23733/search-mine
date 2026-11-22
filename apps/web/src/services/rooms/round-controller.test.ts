// @vitest-environment node
import { expect, test, vi } from "vitest";
import { DEFAULT_RULES, type RoundEvent, type RoundView } from "@liar/protocol";
import { authTestPorts, TEST_ACCOUNT } from "../../../test/auth-ports";
import { RoundController } from "./round-controller";
import type { RoundConnection, RoomPort } from "./types";

test("the first authoritative snapshot binds the seat and ignores old frames and auth owners", async () => {
  const { auth } = authTestPorts(TEST_ACCOUNT, true);
  await auth.refresh();
  const owner = { accountId: TEST_ACCOUNT.id, revision: auth.revision() };
  let emit!: (event: RoundEvent) => void;
  const transport: RoundConnection = {
    listen: (receive) => {
      emit = receive;
    },
    close: vi.fn(),
    send: vi.fn(() => true),
  };
  const port: RoomPort = {
    list: vi.fn(),
    command: vi.fn(),
    connect: vi.fn(async () => transport),
  };
  const roomId = "b0000000-0000-4000-8000-000000000001";
  const roundId = "b0000000-0000-4000-8000-000000000002";
  const round = new RoundController(auth, port, owner, {
    roomId,
    roundId,
    seat: 0,
  });
  round.start();
  await Promise.resolve();
  const view: RoundView = {
    rules: DEFAULT_RULES,
    own: {
      cells: [],
      gauge: 0,
      history: [],
      stun_ms: 0,
      stats: { mistakes: 0, accusation_attempts: 0, correct_accusations: 0 },
    },
    forfeited: false,
    opponents: [],
    revision: 2,
    phase: "playing",
    countdown_ms: 0,
    remaining_ms: 500,
    result: null,
  };
  emit({
    room_id: roomId,
    round_id: roundId,
    v: 2,
    server_seq: 2,
    server_time_ms: 3000,
    payload: {
      type: "snapshot",
      own_seat: 0,
      session_epoch: 5,
      last_client_seq: 2,
      view,
    },
  });
  emit({
    room_id: roomId,
    round_id: roundId,
    v: 2,
    server_seq: 1,
    server_time_ms: 2999,
    payload: { type: "delta", view: { ...view, revision: 1 } },
  });
  round.send({ type: "open", cell: 4 });
  expect(transport.send).toHaveBeenCalledWith(
    expect.objectContaining({
      v: 2,
      round_id: roundId,
      client_seq: 3,
      session_epoch: 5,
      known_revision: 2,
    }),
  );
  auth.invalidate();
  expect(round.read().view).toBeNull();
  expect(transport.close).toHaveBeenCalled();
  round.dispose();
});
