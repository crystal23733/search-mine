import { PROTOCOL_VERSION, type DailyReplay } from "@liar/protocol";
import type {
  AuthenticatedRequests,
  RequestContext,
  RequestOwner,
} from "../auth/requests";
import { AuthError } from "../auth/types";
import { readJsonResponse } from "../json-response";
import {
  dateValue,
  decodeDefinition,
  decodeFailure,
  decodeHeartbeat,
  decodeOwn,
  decodeRankings,
  decodeReplay,
  decodeStart,
  decodeSubmitted,
  metadataValue,
  sameMetadata,
} from "./decode";
import { int, uuid } from "../wire-decode";
import { DailyFailure, type DailyApi } from "./types";
const statuses = {
  daily_invalid: 400,
  auth_required: 401,
  daily_conflict: 409,
  daily_expired: 410,
  daily_rate_limited: 429,
  daily_unavailable: 503,
};
const failures = {
  daily_invalid: "invalid",
  daily_conflict: "conflict",
  daily_expired: "expired",
  daily_rate_limited: "rate_limited",
  daily_unavailable: "unavailable",
} as const;
const versionValue = int(65535, 1);
export function createDailyHttp(
  auth: AuthenticatedRequests,
  fetcher: typeof fetch,
): DailyApi {
  let publicActive = 0;
  async function post<T>(
    path: string,
    body: object,
    signal: AbortSignal,
    decode: (raw: unknown) => T,
    context?: RequestContext,
  ): Promise<T> {
    try {
      if (signal.aborted) throw new DailyFailure("cancelled");
      const encoded = JSON.stringify(body);
      const limit = path === "submit-timed" ? 2 * 1024 * 1024 + 4096 : 4096;
      if (new TextEncoder().encode(encoded).byteLength > limit)
        throw new DailyFailure("invalid");
      const response = await fetcher(`/api/v1/daily/${path}`, {
        method: "POST",
        credentials: context ? "same-origin" : "omit",
        redirect: "error",
        cache: "no-store",
        headers: context
          ? { "content-type": "application/json", "x-liar-csrf": context.csrf }
          : { "content-type": "application/json" },
        body: encoded,
        signal,
      });
      const raw = await readJsonResponse(response, signal, 64 * 1024);
      if (!response.ok) {
        const { code } = decodeFailure(raw);
        if (statuses[code] !== response.status)
          throw new DailyFailure("unavailable");
        if (code === "auth_required") throw new AuthError("auth_required");
        throw new DailyFailure(failures[code]);
      }
      if (response.status !== 200) throw new DailyFailure("unavailable");
      return decode(raw);
    } catch (error) {
      if (error instanceof DailyFailure || error instanceof AuthError)
        throw error;
      throw new DailyFailure(signal.aborted ? "cancelled" : "unavailable");
    }
  }
  const identity = (owner: RequestOwner, c: RequestContext) => ({
    v: PROTOCOL_VERSION,
    account_id: uuid(owner.accountId),
    session_revision: uuid(c.sessionRevision),
  });
  function definition(
    owner: RequestOwner,
    c: RequestContext,
    date: string,
    version: number,
  ) {
    return post(
      "definition",
      {
        ...identity(owner, c),
        date: dateValue(date),
        seed_version: versionValue(version),
      },
      c.signal,
      (raw) => decodeDefinition(raw, date, version),
      c,
    );
  }
  function submit(
    owner: RequestOwner,
    c: RequestContext,
    attempt: string,
    ticket: string | null,
    replay: DailyReplay,
  ) {
    const validated = decodeReplay(replay);
    if (
      new TextEncoder().encode(JSON.stringify(validated)).byteLength >
      2 * 1024 * 1024
    )
      throw new DailyFailure("invalid");
    return post(
      "submit-timed",
      {
        ...identity(owner, c),
        attempt_id: uuid(attempt),
        ticket_id: ticket === null ? null : uuid(ticket),
        replay: validated,
      },
      c.signal,
      (raw) =>
        decodeSubmitted(raw, owner.accountId, attempt, validated.metadata),
      c,
    );
  }
  return {
    start: (owner, attempt, signal) =>
      auth.execute(
        owner,
        (c) =>
          post(
            "start",
            { ...identity(owner, c), attempt_id: uuid(attempt) },
            c.signal,
            (raw) => decodeStart(raw, owner.accountId, attempt),
            c,
          ),
        signal,
      ),
    heartbeat: (owner, ticket, sequence, signal) =>
      auth.execute(
        owner,
        (c) =>
          post(
            "heartbeat",
            {
              ...identity(owner, c),
              ticket_id: uuid(ticket),
              sequence: int(4294967295, 1)(sequence),
            },
            c.signal,
            (raw) => decodeHeartbeat(raw, owner.accountId, ticket, sequence),
            c,
          ),
        signal,
      ),
    definition: (owner, date, version, signal) =>
      auth.execute(owner, (c) => definition(owner, c, date, version), signal),
    submitTimed: (owner, attempt, ticket, replay, signal) =>
      auth.execute(
        owner,
        (c) => submit(owner, c, attempt, ticket, replay),
        signal,
      ),
    own: (owner, metadata, signal) =>
      auth.execute(
        owner,
        (c) =>
          post(
            "own",
            { ...identity(owner, c), metadata: metadataValue(metadata) },
            c.signal,
            (raw) => decodeOwn(raw, owner.accountId, metadata),
            c,
          ),
        signal,
      ),
    submitPending: (owner, record, signal) =>
      auth.execute(
        owner,
        async (c) => {
          // Terminal outcomes remain inside execute so they also require its post-proof.
          try {
            const accepted = await definition(
              owner,
              c,
              record.metadata.date,
              record.metadata.seed_version,
            );
            if (
              !sameMetadata(accepted.metadata, record.metadata) ||
              !sameMetadata(record.metadata, record.replay.metadata)
            )
              return "unsupported";
            await submit(owner, c, record.attempt_id, null, record.replay);
            return "accepted";
          } catch (error) {
            if (error instanceof DailyFailure && error.code === "expired")
              return "expired";
            throw error;
          }
        },
        signal,
      ),
    async rankings(date, version, signal) {
      dateValue(date);
      versionValue(version);
      if (signal?.aborted) throw new DailyFailure("cancelled");
      if (publicActive >= 2) throw new DailyFailure("capacity");
      publicActive++;
      const controller = new AbortController();
      const abort = () => controller.abort();
      signal?.addEventListener("abort", abort, { once: true });
      let interrupted!: () => void;
      const interruption = new Promise<never>((_, reject) => {
        interrupted = () => reject(new DailyFailure("cancelled"));
        controller.signal.addEventListener("abort", interrupted, {
          once: true,
        });
      });
      const timer = setTimeout(abort, 10_000);
      try {
        return await Promise.race([
          post(
            "rankings",
            { v: PROTOCOL_VERSION, date, seed_version: version },
            controller.signal,
            (raw) => decodeRankings(raw, date, version),
          ),
          interruption,
        ]);
      } finally {
        clearTimeout(timer);
        signal?.removeEventListener("abort", abort);
        controller.signal.removeEventListener("abort", interrupted);
        publicActive--;
      }
    },
  };
}
