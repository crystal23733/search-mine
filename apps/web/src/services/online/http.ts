import { responseJson } from "./response";
import type { AuthenticatedRequests } from "../auth/requests";
import { PROTOCOL_VERSION, type LobbyCommand } from "@liar/protocol";
import { AuthError } from "../auth/types";
import { decodeLobby, decodeLobbyFailure } from "./decode";
import { OnlineFailure } from "./types";
import type { OnlinePort } from "./types";
function command(value: LobbyCommand): LobbyCommand {
  switch (value.type) {
    case "status":
      return { type: "status" };
    case "queue_join":
      return { type: "queue_join", difficulty: value.difficulty };
    case "room_create":
      return { type: "room_create" };
    case "room_join":
      return { type: "room_join", code: value.code };
    case "ready":
      return { type: "ready", room_id: value.room_id, ready: value.ready };
    case "cancel": {
      const i = value.identity;
      const identity =
        i.kind === "queue"
          ? { kind: i.kind, queue_id: i.queue_id }
          : i.kind === "room"
            ? { kind: i.kind, room_id: i.room_id }
            : { kind: i.kind, entity: i.entity, generation: i.generation };
      return { type: "cancel", identity };
    }
  }
}
export function createLobbyHttp(
  auth: AuthenticatedRequests,
  fetcher: typeof fetch,
): OnlinePort["lobby"] {
  return (owner, value, signal) =>
    auth.execute(
      owner,
      async (context) => {
        try {
          const response = await fetcher("/api/v1/lobby", {
            method: "POST",
            credentials: "same-origin",
            cache: "no-store",
            redirect: "error",
            signal: context.signal,
            headers: {
              "content-type": "application/json",
              "x-liar-csrf": context.csrf,
            },
            body: JSON.stringify({
              v: PROTOCOL_VERSION,
              command: command(value),
            }),
          });
          const raw = await responseJson(response, context.signal);
          if (response.status !== 200) {
            const failure = decodeLobbyFailure(raw);
            if (response.status === 401 && failure.error === "unauthorized")
              throw new AuthError("auth_required");
            throw new OnlineFailure(failure.error);
          }
          return decodeLobby(raw);
        } catch (error) {
          if (error instanceof AuthError || error instanceof OnlineFailure)
            throw error;
          throw new OnlineFailure("unavailable");
        }
      },
      signal,
    );
}
