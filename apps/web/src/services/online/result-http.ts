import type { AuthenticatedRequests, RequestOwner } from "../auth/requests";
import type { OnlinePort } from "./types";
import { OnlineFailure } from "./types";
import { PROTOCOL_VERSION, type ResultError } from "@liar/protocol";
import { AuthError } from "../auth/types";
import { responseJson } from "./response";
import {
  decodeLatestResult,
  decodePersonalResult,
  decodeResultFailure,
} from "./decode";
const status: Record<ResultError, number> = {
  malformed: 400,
  unsupported_version: 400,
  unauthorized: 401,
  unavailable: 503,
  rate_limited: 429,
  not_found: 404,
};
export function createResultHttp(
  auth: AuthenticatedRequests,
  fetcher: typeof fetch,
): OnlinePort["result"] {
  return (owner, matchId, signal) =>
    readResult(
      auth,
      fetcher,
      owner,
      signal,
      "/api/v1/results",
      { v: PROTOCOL_VERSION, match_id: matchId },
      (raw) => decodePersonalResult(raw, matchId),
      true,
    );
}
export function createLatestResultHttp(
  auth: AuthenticatedRequests,
  fetcher: typeof fetch,
): OnlinePort["latestResult"] {
  return (owner, signal) =>
    readResult(
      auth,
      fetcher,
      owner,
      signal,
      "/api/v1/results/latest",
      { v: PROTOCOL_VERSION },
      decodeLatestResult,
      false,
    );
}
function readResult<T>(
  auth: AuthenticatedRequests,
  fetcher: typeof fetch,
  owner: RequestOwner,
  signal: AbortSignal,
  path: string,
  body: object,
  decode: (raw: unknown) => T,
  allowNotFound: boolean,
): Promise<T> {
  return auth.execute(
    owner,
    async (context) => {
      try {
        const response = await fetcher(path, {
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
        const raw = await responseJson(response, context.signal, 4096);
        if (response.status !== 200) {
          const failure = decodeResultFailure(raw);
          if (status[failure.error] !== response.status)
            throw new OnlineFailure("unavailable");
          if (failure.error === "unauthorized")
            throw new AuthError("auth_required");
          throw new OnlineFailure(
            (allowNotFound && failure.error === "not_found") ||
              failure.error === "rate_limited"
              ? failure.error
              : "unavailable",
          );
        }
        return decode(raw);
      } catch (error) {
        if (
          error instanceof AuthError ||
          (error instanceof OnlineFailure &&
            ["not_found", "rate_limited", "unavailable"].includes(error.code))
        )
          throw error;
        throw new OnlineFailure("unavailable");
      }
    },
    signal,
  );
}
