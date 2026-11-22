import {
  MAX_ROOM_OUTPUT_BYTES,
  ROOM_PROTOCOL_VERSION,
  type RoomIntent,
} from "@liar/protocol";
import type { AuthenticatedRequests } from "../auth/requests";
import { AuthError } from "../auth/types";
import { JsonResponseFailure, readJsonResponse } from "../json-response";
import { decodeRoomResponse } from "./decode";
import { RoomFailure, type RoomPort } from "./types";

export function createRoomsHttp(
  auth: AuthenticatedRequests,
  fetcher: typeof fetch,
): Pick<RoomPort, "list" | "command"> {
  async function request(
    intent: RoomIntent,
    signal: AbortSignal,
    csrf?: string,
  ) {
    let response: Response;
    try {
      response = await fetcher("/api/v2/rooms", {
        method: "POST",
        cache: "no-store",
        redirect: "error",
        signal,
        credentials: csrf ? "same-origin" : "omit",
        headers: {
          "content-type": "application/json",
          ...(csrf ? { "x-liar-csrf": csrf } : {}),
        },
        body: JSON.stringify({ v: ROOM_PROTOCOL_VERSION, intent }),
      });
    } catch {
      throw new RoomFailure("unavailable");
    }
    let payload;
    try {
      payload = decodeRoomResponse(
        await readJsonResponse(response, signal, MAX_ROOM_OUTPUT_BYTES),
      ).payload;
    } catch (error) {
      throw new RoomFailure(
        error instanceof JsonResponseFailure && error.code === "unavailable"
          ? "unavailable"
          : "malformed",
      );
    }
    if (
      response.status === 401 &&
      payload.type === "error" &&
      payload.error === "unauthorized" &&
      csrf
    )
      throw new AuthError("auth_required");
    if (response.status !== 200 && payload.type === "error")
      throw new RoomFailure(payload.error);
    if (response.status !== 200 || payload.type === "error")
      throw new RoomFailure("malformed");
    return payload;
  }
  return {
    async list(cursor, signal) {
      const payload = await request(
        { type: "list", cursor },
        AbortSignal.any([signal, AbortSignal.timeout(2500)]),
      );
      if (payload.type !== "list") throw new RoomFailure("malformed");
      return payload.page;
    },
    command(owner, intent, signal) {
      return auth.execute(
        owner,
        async (context) => {
          const payload = await request(intent, context.signal, context.csrf);
          if (payload.type !== "current") throw new RoomFailure("malformed");
          return payload.room;
        },
        signal,
      );
    },
  };
}
