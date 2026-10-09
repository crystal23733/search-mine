export const RECONNECT_BUDGET_MS = 30_000;
export class BoundedReconnect {
  private generation = 0;
  private until: number | null = null;
  private delay: ReturnType<typeof setTimeout> | undefined;
  private deadline: ReturnType<typeof setTimeout> | undefined;
  private attempt: AbortController | null = null;
  private attemptTimer: ReturnType<typeof setTimeout> | undefined;
  private index = 0;
  constructor(
    private now: () => number,
    private random: () => number,
    private work: (signal: AbortSignal) => Promise<boolean>,
    private exhausted: () => void,
  ) {}
  remaining(): number | null {
    return this.until === null ? null : Math.max(0, this.until - this.now());
  }
  start() {
    if (this.until !== null) return;
    this.until = this.now() + RECONNECT_BUDGET_MS;
    this.index = 0;
    const generation = ++this.generation;
    this.deadline = setTimeout(() => {
      if (generation !== this.generation) return;
      this.stop();
      this.exhausted();
    }, RECONNECT_BUDGET_MS);
    this.schedule(generation);
  }
  private schedule(generation: number) {
    const base = Math.min(8000, 500 * 2 ** Math.min(this.index++, 4));
    const jitter = (Math.max(0, Math.min(1, this.random())) * base) / 4;
    this.delay = setTimeout(
      () => {
        this.delay = undefined;
        void this.run(generation);
      },
      Math.min(base + jitter, this.remaining() ?? 0),
    );
  }
  private async run(generation: number) {
    if (generation !== this.generation || this.until === null) return;
    if (this.now() >= this.until) {
      this.stop();
      this.exhausted();
      return;
    }
    const attempt = new AbortController();
    this.attempt = attempt;
    const timer = setTimeout(
      () => attempt.abort(),
      Math.min(10_000, this.remaining() ?? 0),
    );
    this.attemptTimer = timer;
    let success = false;
    try {
      success = await this.work(attempt.signal);
    } catch {
      /* The caller decides which errors terminate the recovery. */
    } finally {
      clearTimeout(timer);
      if (this.attempt === attempt) {
        this.attempt = null;
        this.attemptTimer = undefined;
      }
    }
    if (generation !== this.generation || this.until === null) return;
    if (this.now() >= this.until) {
      this.stop();
      this.exhausted();
      return;
    }
    if (success && !attempt.signal.aborted) {
      // The accepted connection keeps its signal until the controller ends its lifetime.
      clearTimeout(this.deadline);
      this.deadline = undefined;
      this.until = null;
    } else this.schedule(generation);
  }
  stop() {
    this.generation++;
    clearTimeout(this.delay);
    clearTimeout(this.deadline);
    clearTimeout(this.attemptTimer);
    this.delay = this.deadline = this.attemptTimer = undefined;
    this.until = null;
    this.attempt?.abort();
    this.attempt = null;
  }
}
