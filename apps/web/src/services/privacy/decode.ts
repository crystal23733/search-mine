import type { ConsentSnapshot, OptionalEventReceipt } from "@liar/protocol";
import { decodeMeConsent } from "../me/decode";
import { fail, int, one, optional, struct, uuid } from "../wire-decode";
const timestamp = int(253402300799);
const snapshot = struct({
  v: one(1),
  account_id: uuid,
  observed_at: timestamp,
  revision: optional(uuid),
  consent: (raw: unknown) => raw,
});
export function decodeConsentSnapshot(
  raw: unknown,
  account: string,
): ConsentSnapshot {
  const value = snapshot(raw);
  const consent = decodeMeConsent(value.consent, value.observed_at);
  if (
    value.account_id !== account ||
    (consent.status === "unknown") !== (value.revision === null)
  )
    fail();
  return { ...value, consent };
}
const receipt = struct({ v: one(1), event_id: uuid, received_at: timestamp });
export function decodeEventReceipt(
  raw: unknown,
  id: string,
  occurredAt: number,
): OptionalEventReceipt {
  const value = receipt(raw);
  if (value.event_id !== id || value.received_at < occurredAt) fail();
  return value;
}
export const decodePrivacyFailure = struct({
  error: one(
    "malformed",
    "unsupported_version",
    "unauthorized",
    "unavailable",
    "conflict",
    "rate_limited",
    "consent",
  ),
});
