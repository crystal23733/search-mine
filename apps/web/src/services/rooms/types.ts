import type {
  RoomIntent,
  RoomList,
  RoomListCursor,
  RoomView,
  RoomWireError,
  RoundEvent,
  RoundInput,
} from "@liar/protocol";
import type { RequestOwner } from "../auth/requests";
import type { OnlineConnection } from "../online/types";

export type RoomCommand = Exclude<RoomIntent, { type: "list" }>;
export type RoundConnection = OnlineConnection<RoundInput, RoundEvent>;

export class RoomFailure extends Error {
  constructor(public readonly code: RoomWireError) {
    super(code);
  }
}

export interface RoomPort {
  list(cursor: RoomListCursor | null, signal: AbortSignal): Promise<RoomList>;
  command(
    owner: RequestOwner,
    intent: RoomCommand,
    signal: AbortSignal,
  ): Promise<RoomView | null>;
  connect(
    owner: RequestOwner,
    roomId: string,
    roundId: string,
    signal: AbortSignal,
  ): Promise<RoundConnection>;
}
