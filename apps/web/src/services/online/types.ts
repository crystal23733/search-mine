import type {
  LobbyCommand,
  LobbyResponse,
  OnlineEvent,
  OnlineInput,
} from "@liar/protocol";
import type { RequestOwner } from "../auth/requests";
export class OnlineFailure extends Error {
  constructor(public readonly code: string) {
    super(code);
  }
}
export interface OnlineConnection {
  listen(
    event: (value: OnlineEvent) => void,
    closed: (error: OnlineFailure) => void,
  ): void;
  send(input: OnlineInput): boolean;
  close(): void;
}
export interface OnlinePort {
  lobby(
    owner: RequestOwner,
    command: LobbyCommand,
    signal: AbortSignal,
  ): Promise<LobbyResponse>;
  connect(
    owner: RequestOwner,
    matchId: string,
    signal: AbortSignal,
  ): Promise<OnlineConnection>;
}
