import type { GameView, LocalInput, LocalStep, TutorialView, TutorialStep } from '@liar/protocol';
export type Difficulty = 'easy' | 'normal' | 'hard';
export type CoreRequest = { kind: 'init'; seed: string; difficulty: Difficulty } | { kind: 'snapshot' } | { kind: 'advance'; time_ms: number } | { kind: 'step'; input: LocalInput; time_ms: number } | { kind: 'training-init' } | { kind: 'training-snapshot' } | { kind: 'training-advance'; time_ms: number } | { kind: 'training-step'; input: LocalInput; time_ms: number };
type PublicReply = GameView | LocalStep | TutorialView | TutorialStep;
export type CoreReply = { id: number; ok: PublicReply } | { id: number; error: string };
export type WorkerPort = Pick<Worker, 'postMessage' | 'addEventListener' | 'removeEventListener' | 'terminate'>;
export interface PracticeCore {
  init(seed: string, difficulty: Difficulty): Promise<GameView>;
  snapshot(): Promise<GameView>;
  advance(time_ms: number): Promise<GameView>;
  step(input: LocalInput, time_ms: number): Promise<LocalStep>;
  dispose(): void;
}
export interface TrainingCore {
  init(): Promise<TutorialView>;
  snapshot(): Promise<TutorialView>;
  advance(time_ms: number): Promise<TutorialView>;
  step(input: LocalInput, time_ms: number): Promise<TutorialStep>;
  dispose(): void;
}
class WorkerRpc {
  private serial = 0;
  private pending = new Map<number, { resolve: (value: PublicReply) => void; reject: (reason: Error) => void; timer: ReturnType<typeof setTimeout> }>();
  private disposed = false;
  private readonly onMessage = (event: MessageEvent<CoreReply>) => {
    const reply = event.data;
    const pending = this.pending.get(reply.id);
    if (!pending) return;
    clearTimeout(pending.timer);
    this.pending.delete(reply.id);
    if ('error' in reply) pending.reject(new Error(reply.error));
    else pending.resolve(reply.ok);
  };
  private readonly onError = () => this.dispose();
  constructor(private worker: WorkerPort, private timeout_ms = 30_000) {
    worker.addEventListener('message', this.onMessage);
    worker.addEventListener('error', this.onError);
  }
  protected request<T extends PublicReply>(request: CoreRequest): Promise<T> {
    if (this.disposed) return Promise.reject(new Error('disposed'));
    if (this.pending.size >= 64) return Promise.reject(new Error('busy'));
    const id = ++this.serial;
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id); reject(new Error('timeout')); }, this.timeout_ms);
      this.pending.set(id, { resolve: value => resolve(value as T), reject, timer });
      try { this.worker.postMessage({ id, ...request }); } catch { clearTimeout(timer); this.pending.delete(id); reject(new Error('unavailable')); }
    });
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.worker.removeEventListener('message', this.onMessage);
    this.worker.removeEventListener('error', this.onError);
    this.worker.terminate();
    for (const pending of this.pending.values()) { clearTimeout(pending.timer); pending.reject(new Error('disposed')); }
    this.pending.clear();
  }
}
export class WorkerPracticeCore extends WorkerRpc implements PracticeCore {
  init(seed: string, difficulty: Difficulty) { return this.request<GameView>({ kind: 'init', seed, difficulty }); }
  snapshot() { return this.request<GameView>({ kind: 'snapshot' }); }
  advance(time_ms: number) { return this.request<GameView>({ kind: 'advance', time_ms }); }
  step(input: LocalInput, time_ms: number) { return this.request<LocalStep>({ kind: 'step', input, time_ms }); }
}
export class WorkerTrainingCore extends WorkerRpc implements TrainingCore {
  init() { return this.request<TutorialView>({ kind: 'training-init' }); }
  snapshot() { return this.request<TutorialView>({ kind: 'training-snapshot' }); }
  advance(time_ms: number) { return this.request<TutorialView>({ kind: 'training-advance', time_ms }); }
  step(input: LocalInput, time_ms: number) { return this.request<TutorialStep>({ kind: 'training-step', input, time_ms }); }
}
