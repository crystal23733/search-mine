import type { AuthenticatedRequests, RequestOwner } from "../auth/requests";
import { AuthError } from "../auth/types";
import { readJsonResponse } from "../json-response";
import { decodeMeOverview, decodeMeHistory, decodeMeFailure } from "./decode";
import { MeFailure, type MePort } from "./types";
export function createMeHttp(
  auth: AuthenticatedRequests,
  fetcher: typeof fetch,
): MePort {
  function read<T>(
    owner: RequestOwner,
    signal: AbortSignal,
    path: string,
    body: object,
    decode: (raw: unknown) => T,
  ): Promise<T> {
    return auth.execute(
      owner,
      async (context) => {
        try {
          const response = await fetcher(`/api/v1/me/${path}`, {
            method: "POST",
            credentials: "same-origin",
            cache: "no-store",
            redirect: "error",
            signal: context.signal,
            headers: {
              "content-type": "application/json",
              "x-liar-csrf": context.csrf,
            },
            body: JSON.stringify(body),
          });
          const raw = await readJsonResponse(response, context.signal, 65536);
          if (response.status !== 200) {
            const failure = decodeMeFailure(raw);
            if (response.status === 401 && failure.error === "unauthorized")
              throw new AuthError("auth_required");
            if (response.status === 429 && failure.error === "rate_limited")
              throw new MeFailure("rate_limited");
            throw new MeFailure();
          }
          return decode(raw);
        } catch (error) {
          if (
            error instanceof MeFailure ||
            (error instanceof AuthError && error.code === "auth_required")
          )
            throw error;
          throw new MeFailure();
        }
      },
      signal,
      "self",
    );
  }
  return {
    overview: (owner, signal) =>
      read(owner, signal, "overview", { v: 1 }, (raw) =>
        decodeMeOverview(raw, owner.accountId),
      ),
    history: (owner, selection, signal) => {
      if (
        !["online", "daily"].includes(selection.kind) ||
        !["all", "human", "bot"].includes(selection.opponent) ||
        (selection.kind === "daily" && selection.opponent !== "all") ||
        (selection.cursor !== null &&
          !/^[A-Za-z0-9_-]{1,512}$/.test(selection.cursor))
      )
        return Promise.reject(new MeFailure());
      return read(
        owner,
        signal,
        "history",
        {
          v: 1,
          kind: selection.kind,
          opponent: selection.opponent,
          cursor: selection.cursor,
        },
        (raw) => decodeMeHistory(raw, owner.accountId, selection),
      );
    },
  };
}
