import type {
  ConsentSnapshot,
  ConsentWithdrawal,
  OptionalEventAction,
  OptionalEventCaptureInput,
  OptionalEventReceipt,
} from "@liar/protocol";
import type { RequestOwner } from "../auth/requests";
import type { Locale } from "../locale";
export interface ConsentControlPort {
  read(owner: RequestOwner, signal: AbortSignal): Promise<ConsentSnapshot>;
  refresh(
    owner: RequestOwner,
    expected: string | null,
    signal: AbortSignal,
  ): Promise<ConsentSnapshot>;
  withdraw(
    owner: RequestOwner,
    purpose: ConsentWithdrawal,
    signal: AbortSignal,
  ): Promise<ConsentSnapshot>;
}
export interface EventTransport {
  send(
    owner: RequestOwner,
    input: OptionalEventCaptureInput,
    signal: AbortSignal,
  ): Promise<OptionalEventReceipt>;
}
export interface OptionalEvents {
  record(action: OptionalEventAction<string>, locale: Locale): Promise<boolean>;
  cancel(): void;
  dispose(): void;
}
export class PrivacyFailure extends Error {
  constructor(
    public readonly code:
      "unavailable" | "conflict" | "rate_limited" | "consent" = "unavailable",
  ) {
    super(code);
  }
}
