import type { PersonalResult } from "@liar/protocol";
import type { AuthPort } from "../auth/types";
import type { RequestOwner } from "../auth/requests";
import type { OnlinePort } from "./types";
export interface LatestResultState {
  status: "idle" | "loading" | "ready" | "empty" | "error" | "unauthorized";
  result: PersonalResult | null;
}
type ResultAuth = Pick<
  AuthPort,
  "account" | "revision" | "connected" | "subscribe" | "invalidate"
>;
export class LatestResultController {
  private state: LatestResultState = { status: "idle", result: null };
  private owner: RequestOwner | null = null;
  private request: AbortController | null = null;
  private disposed = false;
  private listeners = new Set<() => void>();
  private unsubscribe: () => void;
  constructor(
    private auth: ResultAuth,
    private port: Pick<OnlinePort, "latestResult">,
  ) {
    this.unsubscribe = auth.subscribe(() => {
      if (this.owner && !this.currentOwner()) {
        this.cancel();
        this.owner = null;
        this.publish("unauthorized");
      }
    });
  }
  read() {
    return this.state;
  }
  subscribe(listener: () => void) {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
  private currentOwner() {
    return (
      !this.disposed &&
      this.owner !== null &&
      this.auth.connected() &&
      this.auth.account() === this.owner.accountId &&
      this.auth.revision() === this.owner.revision
    );
  }
  private publish(
    status: LatestResultState["status"],
    result: PersonalResult | null = null,
  ) {
    this.state = { status, result };
    for (const listener of this.listeners) listener();
  }
  private cancel() {
    this.request?.abort();
    this.request = null;
  }
  async start() {
    if (this.disposed) return;
    this.cancel();
    const accountId = this.auth.account();
    if (!accountId || !this.auth.connected()) {
      this.owner = null;
      this.publish("unauthorized");
      return;
    }
    this.owner = { accountId, revision: this.auth.revision() };
    const request = new AbortController();
    this.request = request;
    const current = () =>
      this.request === request &&
      !request.signal.aborted &&
      this.currentOwner();
    this.publish("loading");
    try {
      const result = await this.port.latestResult(this.owner, request.signal);
      if (current()) this.publish(result === null ? "empty" : "ready", result);
    } catch (error) {
      if (!current()) return;
      const code =
        error instanceof Error && "code" in error ? error.code : null;
      if (code === "auth_required" || code === "unauthorized")
        this.auth.invalidate();
      else this.publish("error");
    } finally {
      if (this.request === request) this.request = null;
    }
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.cancel();
    this.owner = null;
    this.unsubscribe();
    this.publish("unauthorized");
    this.listeners.clear();
  }
}
