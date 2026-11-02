import type {
  MeHistory,
  MeKind,
  MeOpponentFilter,
  MeOverview,
} from "@liar/protocol";
import type { RequestOwner } from "../auth/requests";
export interface MeSelection {
  kind: MeKind;
  opponent: MeOpponentFilter;
  cursor: string | null;
}
export interface MePort {
  overview(owner: RequestOwner, signal: AbortSignal): Promise<MeOverview>;
  history(
    owner: RequestOwner,
    selection: MeSelection,
    signal: AbortSignal,
  ): Promise<MeHistory>;
}
export class MeFailure extends Error {
  constructor(
    public readonly code: "unavailable" | "rate_limited" = "unavailable",
  ) {
    super(code);
  }
}
