import type {
  DailyDefinition,
  DailyHeartbeatResponse,
  DailyMetadata,
  DailyOwnResponse,
  DailyRankingsResponse,
  DailyReplay,
  DailyStartResponse,
  DailySubmitResponse,
} from "@liar/protocol";
import type { RequestOwner } from "../auth/requests";
import type { DailyRecord } from "../daily-records";
export class DailyFailure extends Error {
  constructor(
    public readonly code:
      | "invalid"
      | "conflict"
      | "expired"
      | "rate_limited"
      | "unavailable"
      | "cancelled"
      | "capacity",
  ) {
    super(code);
  }
}
export interface DailyApi {
  start(
    owner: RequestOwner,
    attempt: string,
    signal?: AbortSignal,
  ): Promise<DailyStartResponse>;
  heartbeat(
    owner: RequestOwner,
    ticket: string,
    sequence: number,
    signal?: AbortSignal,
  ): Promise<DailyHeartbeatResponse>;
  definition(
    owner: RequestOwner,
    date: string,
    version: number,
    signal?: AbortSignal,
  ): Promise<DailyDefinition>;
  submitTimed(
    owner: RequestOwner,
    attempt: string,
    ticket: string | null,
    replay: DailyReplay,
    signal?: AbortSignal,
  ): Promise<DailySubmitResponse>;
  own(
    owner: RequestOwner,
    metadata: DailyMetadata,
    signal?: AbortSignal,
  ): Promise<DailyOwnResponse>;
  rankings(
    date: string,
    version: number,
    signal?: AbortSignal,
  ): Promise<DailyRankingsResponse>;
  submitPending(
    owner: RequestOwner,
    record: DailyRecord,
    signal: AbortSignal,
  ): Promise<"accepted" | "expired" | "unsupported">;
}
