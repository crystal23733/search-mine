import { DAILY_SEED_VERSION, type DailyRankingsResponse } from "@liar/protocol";
import { utcDailyDate } from "../daily-session";
import type { DailyApi } from "./types";
export interface DailyRankingsState {
  status: "idle" | "loading" | "ready" | "error" | "offline";
  date: string;
  result: DailyRankingsResponse | null;
}
export class DailyRankingsController {
  private state: DailyRankingsState = {
    status: "idle",
    date: "",
    result: null,
  };
  private request: AbortController | null = null;
  private online = false;
  private disposed = false;
  private listeners = new Set<() => void>();
  constructor(
    private rankings: DailyApi["rankings"],
    private wallClock: () => number,
  ) {}
  read() {
    return this.state;
  }
  subscribe(listener: () => void) {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }
  private publish(state: DailyRankingsState) {
    if (this.disposed) return;
    this.state = state;
    for (const listener of this.listeners) listener();
  }
  private cancel() {
    this.request?.abort();
    this.request = null;
  }
  async setOnline(online: boolean) {
    if (this.disposed || this.online === online) {
      if (!this.disposed && !online)
        this.publish({ ...this.state, status: "offline", result: null });
      return;
    }
    this.online = online;
    if (online) await this.load();
    else {
      this.cancel();
      this.publish({ ...this.state, status: "offline", result: null });
    }
  }
  async load() {
    if (this.disposed) return;
    this.cancel();
    if (!this.online) {
      this.publish({ ...this.state, status: "offline", result: null });
      return;
    }
    const date = utcDailyDate(this.wallClock());
    if (!date) {
      this.publish({ status: "error", date, result: null });
      return;
    }
    const request = new AbortController();
    this.request = request;
    const current = () =>
      !this.disposed &&
      this.online &&
      this.request === request &&
      !request.signal.aborted;
    this.publish({ status: "loading", date, result: null });
    try {
      const result = await this.rankings(
        date,
        DAILY_SEED_VERSION,
        request.signal,
      );
      if (current()) this.publish({ status: "ready", date, result });
    } catch {
      if (current()) this.publish({ status: "error", date, result: null });
    } finally {
      if (this.request === request) this.request = null;
    }
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.cancel();
    this.listeners.clear();
  }
}
