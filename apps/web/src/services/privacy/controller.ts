import type { ConsentSnapshot, ConsentWithdrawal } from "@liar/protocol";
import type { AuthPort } from "../auth/types";
import type { RequestOwner } from "../auth/requests";
import { PrivacyFailure, type ConsentControlPort } from "./types";
export interface ConsentControlState {
  status: "idle" | "loading" | "ready" | "error" | "unauthorized";
  snapshot: ConsentSnapshot | null;
  startedAt: number;
  error: PrivacyFailure["code"] | null;
}
export class ConsentController {
  private state: ConsentControlState = {
    status: "idle",
    snapshot: null,
    startedAt: 0,
    error: null,
  };
  private listeners = new Set<() => void>();
  private request: AbortController | null = null;
  private owner: RequestOwner | null = null;
  private disposed = false;
  private remove: () => void;
  constructor(
    private auth: AuthPort,
    private api: ConsentControlPort,
    private clock = () => performance.now(),
  ) {
    this.remove = auth.subscribe(() => {
      if (this.owner && !this.current()) {
        this.cancel();
        this.publish("unauthorized");
      }
    });
  }
  read = () => this.state;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  private current() {
    return (
      !this.disposed &&
      this.owner !== null &&
      this.auth.connected() &&
      !this.auth.read().working &&
      this.auth.account() === this.owner.accountId &&
      this.auth.revision() === this.owner.revision
    );
  }
  private publish(
    status: ConsentControlState["status"],
    snapshot: ConsentSnapshot | null = null,
    error: ConsentControlState["error"] = null,
  ) {
    this.state = { ...this.state, status, snapshot, error };
    for (const fn of this.listeners) fn();
  }
  cancel() {
    this.request?.abort();
    this.request = null;
    this.publish("idle");
  }
  async load() {
    return this.run("read");
  }
  async refresh() {
    return this.run("refresh");
  }
  async withdraw(purpose: ConsentWithdrawal) {
    return this.run(purpose);
  }
  private async run(operation: "read" | "refresh" | ConsentWithdrawal) {
    if (this.disposed || this.request) return false;
    const accountId = this.auth.account();
    if (!accountId || !this.auth.connected() || this.auth.read().working) {
      this.publish("unauthorized");
      return false;
    }
    const expected = this.state.snapshot?.revision ?? null;
    this.owner = { accountId, revision: this.auth.revision() };
    const owner = this.owner;
    const request = new AbortController();
    this.request = request;
    this.state = { ...this.state, startedAt: this.clock() };
    this.publish("loading");
    try {
      const value = await (operation === "read"
        ? this.api.read(owner, request.signal)
        : operation === "refresh"
          ? this.api.refresh(owner, expected, request.signal)
          : this.api.withdraw(owner, operation, request.signal));
      if (request.signal.aborted || this.request !== request || this.disposed)
        return false;
      if (!this.current()) {
        this.publish("unauthorized");
        return false;
      }
      if (value.account_id !== owner.accountId) throw new PrivacyFailure();
      this.publish("ready", value);
      return true;
    } catch (error) {
      if (!request.signal.aborted && this.request === request && this.current())
        this.publish(
          "error",
          null,
          error instanceof PrivacyFailure ? error.code : "unavailable",
        );
      return false;
    } finally {
      if (this.request === request) this.request = null;
    }
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.request?.abort();
    this.request = null;
    this.owner = null;
    this.remove();
    this.publish("unauthorized");
    this.listeners.clear();
  }
}
