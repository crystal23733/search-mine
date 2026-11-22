import type { RoundEvent, RoundInput } from "@liar/protocol";
import type { AuthenticatedRequests } from "../auth/requests";
import { connectSocket, type SocketFactory } from "../online/socket";
import { decodeRoundEvent } from "./round-wire";
import type { RoomPort } from "./types";

export function createRoundSocket(
  auth: AuthenticatedRequests,
  origin: string,
  factory: SocketFactory,
): RoomPort["connect"] {
  return (owner, roomId, roundId, signal) =>
    connectSocket<RoundInput, RoundEvent>(
      auth,
      origin,
      factory,
      owner,
      signal,
      {
        path: "/api/v2/ws",
        decode(raw) {
          const event = decodeRoundEvent(raw);
          if (event.room_id !== roomId || event.round_id !== roundId)
            throw Error();
          return event;
        },
        initial: ({ payload }) =>
          payload.type === "awaiting" ||
          payload.type === "snapshot" ||
          payload.type === "end",
        failure: ({ payload }) =>
          payload.type === "error" ? payload.code : undefined,
        payload(input) {
          if (input.round_id !== roundId) return null;
          const {
            v,
            command_id,
            client_seq,
            session_epoch,
            known_revision,
            action: a,
          } = input;
          const action =
            a.type === "attack"
              ? { type: a.type }
              : { type: a.type, cell: a.cell };
          return {
            v,
            round_id: roundId,
            command_id,
            client_seq,
            session_epoch,
            known_revision,
            action,
          };
        },
      },
    );
}
