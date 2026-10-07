import type { LocalInput, LocalAck, PublicAction } from "@liar/protocol";
export interface SessionPort<View> {
  init(): Promise<View>;
  advance(time: number): Promise<View>;
  step(input: LocalInput, time: number): Promise<{ ack: LocalAck; view: View }>;
  dispose(): void;
}
export interface SessionClock {
  now(): number;
  repeat(callback: () => void, ms: number): () => void;
}
export type SessionState<View> = {
  status: "loading" | "ready" | "error";
  view: View | null;
  error: string | null;
};
export const browserClock: SessionClock = {
  now: () => performance.now(),
  repeat: (callback, ms) => {
    const id = setInterval(callback, ms);
    return () => clearInterval(id);
  },
};
export class LocalController<View> {
  private state: SessionState<View> = {
    status: "loading",
    view: null,
    error: null,
  };
  private core?: SessionPort<View>;
  private disposed = false;
  private started = false;
  private origin = 0;
  private serial = 0;
  private pending = 0;
  private tickPending = false;
  private queue = Promise.resolve();
  private stopTimer?: () => void;
  private lastInput?: LocalInput;
  private listeners = new Set<() => void>();
  constructor(
    private factory: () => Promise<SessionPort<View>>,
    private clock: SessionClock,
    private finished: (view: View) => boolean,
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
  private publish(state: SessionState<View>) {
    if (this.disposed) return;
    this.state = state;
    if (state.view !== null && this.finished(state.view)) this.cancelTimer();
    for (const listener of this.listeners) listener();
  }
  private cancelTimer() {
    this.stopTimer?.();
    this.stopTimer = undefined;
  }
  private fail(error: unknown) {
    this.cancelTimer();
    this.publish({
      ...this.state,
      status: "error",
      error:
        error instanceof Error &&
        /^(invalid_time|timeout|busy|disposed)$/.test(error.message)
          ? error.message
          : "unavailable",
    });
    this.core?.dispose();
    this.core = undefined;
  }
  async start() {
    if (this.started || this.disposed) return;
    this.started = true;
    try {
      const core = await this.factory();
      if (this.disposed) {
        core.dispose();
        return;
      }
      this.core = core;
      const view = await core.init();
      if (this.disposed) return;
      this.origin = this.clock.now();
      this.publish({ status: "ready", view, error: null });
      if (!this.finished(view))
        this.stopTimer = this.clock.repeat(() => this.tick(), 250);
    } catch (error) {
      if (!this.disposed) this.fail(error);
    }
  }
  private active() {
    return (
      !this.disposed &&
      this.state.status === "ready" &&
      this.state.view !== null &&
      !this.finished(this.state.view)
    );
  }
  private enqueue(
    task: (core: SessionPort<View>, time: number) => Promise<void>,
  ) {
    if (!this.active()) return this.queue;
    if (this.pending >= 64) {
      this.publish({ ...this.state, error: "busy" });
      return this.queue;
    }
    this.pending++;
    this.queue = this.queue
      .then(async () => {
        if (!this.active() || !this.core) return;
        const time = Math.floor(this.clock.now() - this.origin);
        if (!Number.isSafeInteger(time) || time < 0 || time > 0xffffffff)
          throw Error("invalid_time");
        await task(this.core, time);
      })
      .catch((error) => {
        if (!this.disposed) this.fail(error);
      })
      .finally(() => {
        this.pending--;
      });
    return this.queue;
  }
  private tick() {
    if (this.tickPending || !this.active()) return;
    this.tickPending = true;
    void this.enqueue(async (core, time) => {
      const view = await core.advance(time);
      this.publish({ status: "ready", view, error: this.state.error });
    }).finally(() => {
      this.tickPending = false;
    });
  }
  private input(input: LocalInput) {
    return this.enqueue(async (core, time) => {
      const step = await core.step(input, time);
      this.publish({ status: "ready", view: step.view, error: step.ack.error });
    });
  }
  submit(action: PublicAction) {
    if (!this.active() || this.pending >= 64)
      return this.enqueue(async () => {});
    const serial = ++this.serial;
    this.lastInput = { v: 1, command_id: serial, client_seq: serial, action };
    return this.input(this.lastInput);
  }
  resendLast() {
    return this.lastInput ? this.input(this.lastInput) : Promise.resolve();
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.cancelTimer();
    this.core?.dispose();
    this.core = undefined;
    this.listeners.clear();
  }
}
export function localSeed(): string {
  const words = crypto.getRandomValues(new Uint32Array(2));
  return ((BigInt(words[0]) << 32n) | BigInt(words[1])).toString();
}
