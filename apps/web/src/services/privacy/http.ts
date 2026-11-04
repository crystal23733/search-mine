import type { AuthenticatedRequests, RequestOwner } from "../auth/requests";
import { AuthError } from "../auth/types";
import { readJsonResponse } from "../json-response";
import {
  decodeConsentSnapshot,
  decodeEventReceipt,
  decodePrivacyFailure,
} from "./decode";
import {
  PrivacyFailure,
  type ConsentControlPort,
  type EventTransport,
} from "./types";
function post<T>(
  auth: AuthenticatedRequests,
  fetcher: typeof fetch,
  owner: RequestOwner,
  signal: AbortSignal,
  path: string,
  input: object,
  decode: (raw: unknown) => T,
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
          body: JSON.stringify(input),
        });
        const raw = await readJsonResponse(response, context.signal, 4096);
        if (response.status !== 200) {
          const { error } = decodePrivacyFailure(raw);
          if (response.status === 401 && error === "unauthorized")
            throw new AuthError("auth_required");
          if (response.status === 409 && error === "conflict")
            throw new PrivacyFailure("conflict");
          if (response.status === 429 && error === "rate_limited")
            throw new PrivacyFailure("rate_limited");
          if (response.status === 403 && error === "consent")
            throw new PrivacyFailure("consent");
          throw new PrivacyFailure();
        }
        return decode(raw);
      } catch (error) {
        if (error instanceof PrivacyFailure || error instanceof AuthError)
          throw error;
        throw new PrivacyFailure();
      }
    },
    signal,
    "self",
  );
}
export function createConsentHttp(
  auth: AuthenticatedRequests,
  fetcher: typeof fetch,
  mutation: () => void = () => {},
): ConsentControlPort {
  const request = (
    owner: RequestOwner,
    signal: AbortSignal,
    path: string,
    input: object,
  ) =>
    post(
      auth,
      fetcher,
      owner,
      signal,
      `/api/v1/me/consent${path}`,
      input,
      (raw) => decodeConsentSnapshot(raw, owner.accountId),
    );
  return {
    read: (owner, signal) => request(owner, signal, "", { v: 1 }),
    refresh: (owner, expected, signal) => {
      mutation();
      return request(owner, signal, "/refresh", {
        v: 1,
        expected_revision: expected,
      }).then((value) => {
        if (
          value.consent.status !== "current" ||
          value.revision === null ||
          (expected !== null && value.revision === expected)
        )
          throw new PrivacyFailure();
        return value;
      });
    },
    withdraw: (owner, purpose, signal) => {
      mutation();
      return request(owner, signal, "/withdraw", { v: 1, purpose }).then(
        (value) => {
          if (
            value.consent.status !== "current" ||
            (purpose === "all"
              ? [value.consent.analytics, value.consent.advertising]
              : [value.consent[purpose]]
            ).some((choice) => choice !== "denied")
          )
            throw new PrivacyFailure();
          return value;
        },
      );
    },
  };
}
export function createEventHttp(
  auth: AuthenticatedRequests,
  fetcher: typeof fetch,
): EventTransport {
  return {
    send: (owner, input, signal) =>
      post(
        auth,
        fetcher,
        owner,
        signal,
        "/api/v1/events/optional",
        input,
        (raw) =>
          decodeEventReceipt(
            raw,
            input.draft.event_id,
            input.draft.occurred_at,
          ),
      ),
  };
}
