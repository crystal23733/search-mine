import type {
  MeOverview,
  MeHistory,
  MeKind,
  MeOpponentFilter,
} from "@liar/protocol";
import type { AuthPort } from "../auth/types";
import { MeFailure, type MePort } from "./types";
import type { RequestOwner } from "../auth/requests";
export interface MeState {
  startedAt: number;
  status: "idle" | "loading" | "ready" | "empty" | "error" | "unauthorized";
  overview: MeOverview | null;
  history: MeHistory | null;
  kind: MeKind;
  opponent: MeOpponentFilter;
  page: number;
  error: "unavailable" | "rate_limited" | null;
}
export class MeController {
  private state: MeState = {
    startedAt: 0,
    status: "idle",
    overview: null,
    history: null,
    kind: "online",
    opponent: "all",
    page: 0,
    error: null,
  };
  private owner: RequestOwner | null = null;
  private request: AbortController | null = null;
  private cursors: (string | null)[] = [null];
  private disposed = false;
  private resumeOwner: string | null = null;
  private listeners = new Set<() => void>();
  private unsubscribe: () => void;
  constructor(
    private auth: AuthPort,
    private port: MePort,
    private mode: "overview" | "history" = "overview",
  ) {
    this.unsubscribe = auth.subscribe(() => {
      const state = auth.read();
      if (this.owner && !this.currentOwner()) {
        const transient =
          state.status === "ready" &&
          state.working &&
          state.account?.id === this.owner.accountId;
        this.resumeOwner = transient ? this.owner.accountId : null;
        this.cancel();
        this.owner = null;
        this.cursors = [null];
        this.state = { ...this.state, page: 0 };
        this.publish(transient ? "loading" : "unauthorized");
      } else if (this.resumeOwner) {
        if (
          state.account?.id !== this.resumeOwner ||
          state.status !== "ready"
        ) {
          this.resumeOwner = null;
          this.publish("unauthorized");
        } else if (!state.working && auth.connected()) {
          this.resumeOwner = null;
          void this.start();
        }
      }
    });
  }
  read() {
    return this.state;
  }
  subscribe(listener: () => void) {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
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
  private cancel() {
    this.request?.abort();
    this.request = null;
  }
  private publish(
    status: MeState["status"],
    overview: MeOverview | null = null,
    history: MeHistory | null = null,
    error: MeState["error"] = null,
  ) {
    this.state = { ...this.state, status, overview, history, error };
    for (const listener of this.listeners) listener();
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
    const owner = { accountId, revision: this.auth.revision() };
    this.owner = owner;
    const request = new AbortController();
    this.request = request;
    const current = () =>
      this.request === request &&
      !request.signal.aborted &&
      this.currentOwner();
    const selection = {
      kind: this.state.kind,
      opponent: this.state.opponent,
      cursor: this.cursors[this.state.page],
    };
    this.state = { ...this.state, startedAt: performance.now() };
    this.publish("loading");
    try {
      if (this.mode === "overview") {
        const value = await this.port.overview(owner, request.signal);
        if (!current()) return;
        if (value.account.id !== owner.accountId) throw new MeFailure();
        this.publish("ready", value);
      } else {
        const value = await this.port.history(owner, selection, request.signal);
        if (!current()) return;
        if (
          value.account_id !== owner.accountId ||
          value.kind !== selection.kind ||
          value.opponent !== selection.opponent
        )
          throw new MeFailure();
        this.publish(value.items.length === 0 ? "empty" : "ready", null, value);
      }
    } catch (error) {
      if (!current()) return;
      const code =
        error instanceof Error && "code" in error ? error.code : null;
      if (code === "auth_required") {
        this.auth.invalidate();
        return;
      }
      this.publish(
        "error",
        null,
        null,
        code === "rate_limited" ? "rate_limited" : "unavailable",
      );
    } finally {
      if (this.request === request) this.request = null;
    }
  }
  async filter(kind: MeKind, opponent: MeOpponentFilter) {
    if (this.disposed || this.mode !== "history") return;
    this.cursors = [null];
    this.state = {
      ...this.state,
      kind,
      opponent: kind === "daily" ? "all" : opponent,
      page: 0,
    };
    await this.start();
  }
  async next() {
    const next = this.state.history?.next;
    if (this.disposed || this.state.status !== "ready" || !next) return;
    if (this.cursors.includes(next) || this.state.page >= 499) {
      this.publish("error", null, null, "unavailable");
      return;
    }
    this.cursors = this.cursors.slice(0, this.state.page + 1);
    this.cursors.push(next);
    this.state = { ...this.state, page: this.state.page + 1 };
    await this.start();
  }
  async back() {
    if (this.disposed || this.state.page === 0 || this.mode !== "history")
      return;
    this.state = { ...this.state, page: this.state.page - 1 };
    await this.start();
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.resumeOwner = null;
    this.cancel();
    this.owner = null;
    this.cursors = [null];
    this.unsubscribe();
    this.publish("unauthorized");
    this.listeners.clear();
  }
}
