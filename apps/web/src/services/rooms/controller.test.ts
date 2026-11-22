// @vitest-environment node
import { expect, test, vi } from "vitest";
import type { RoomView } from "@liar/protocol";
import { authTestPorts, TEST_ACCOUNT } from "../../../test/auth-ports";
import { RoomsController } from "./controller";
import type { RoomPort } from "./types";

const id = "b0000000-0000-4000-8000-000000000001";
const room: RoomView = {
  room_id: id,
  revision: "1",
  code: "ABCD2345",
  phase: "waiting",
  expires_at_ms: 600000,
  options: { title: "Evening race", visibility: "public", capacity: 4 },
  own_seat: 0,
  host_seat: 0,
  members: [{ seat: 0, nickname: "Detective", ready: false, in_round: false }],
  round: null,
};

test("a late status response cannot replace a newer mutation, and sign-out clears the private room", async () => {
  const { auth } = authTestPorts(TEST_ACCOUNT, true);
  await auth.refresh();
  let deliver!: (room: RoomView | null) => void;
  const status = new Promise<RoomView | null>((resolve) => {
    deliver = resolve;
  });
  const command = vi
    .fn<RoomPort["command"]>(async () => room)
    .mockReturnValueOnce(status);
  const api: RoomPort = {
    command,
    list: vi.fn(async () => ({ rooms: [], next: null })),
    connect: vi.fn(),
  };
  const controller = new RoomsController(auth, api, () => true);
  controller.start();
  await controller.mutate({
    type: "create",
    request_id: id,
    options: room.options,
  });
  expect(controller.read().room).toEqual(room);
  deliver(null);
  await status;
  expect(controller.read().room).toEqual(room);
  auth.invalidate();
  expect(controller.read().room).toBeNull();
  controller.dispose();
});
