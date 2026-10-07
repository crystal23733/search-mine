import type { CoreReply, CoreRequest } from './index';
import type { GameView, LocalStep, TutorialView, TutorialStep, DailyView, DailyStep, DailyReplay } from '@liar/protocol';
import type { LocalSession, TrainingSession, DailySession } from './wasm-bindings';
type Request = CoreRequest & { id: number };
const scope = self as unknown as { onmessage: ((event: MessageEvent<Request>) => void) | null; postMessage: (reply: CoreReply) => void };
let session: LocalSession | TrainingSession | DailySession | undefined;
let mode: 'practice' | 'training' | 'daily' = 'practice';
let queue = Promise.resolve();
async function handle(request: Request) {
  try {
    if (!Number.isSafeInteger(request.id) || request.id <= 0) return;
    const requestedMode = request.kind.startsWith('training-') ? 'training' : request.kind.startsWith('daily-') ? 'daily' : 'practice';
    if (request.kind === 'init' || request.kind === 'training-init' || request.kind === 'daily-init') {
      if (session) throw new Error('unavailable');
      const uri = new URL('/core/liar_wasm.js', self.location.origin).href;
      const wasm = await import(/* @vite-ignore */ uri) as typeof import('./wasm-bindings');
      await wasm.default();
      mode = requestedMode;
      session = request.kind === 'daily-init' ? new wasm.DailySession(request.date, request.seed_version) : request.kind === 'training-init' ? new wasm.TrainingSession() : new wasm.LocalSession(request.seed, request.difficulty);
    }
    if (!session) throw new Error('not_started');
    if (requestedMode !== mode) throw new Error('unavailable');
    if ('time_ms' in request) {
      if (!Number.isInteger(request.time_ms) || request.time_ms < 0 || request.time_ms > 0xffffffff) throw new Error('invalid_time');
    }
    const value = request.kind === 'daily-replay' && 'replay' in session ? session.replay() : 'input' in request ? session.step(JSON.stringify(request.input), request.time_ms) : 'time_ms' in request ? session.advance(request.time_ms) : session.snapshot();
    scope.postMessage({ id: request.id, ok: JSON.parse(value) as GameView | LocalStep | TutorialView | TutorialStep | DailyView | DailyStep | DailyReplay });
  } catch (error) {
    // Rust adapter errors contain only stable public error codes. Other JS failures are generic.
    const message = error instanceof Error ? error.message : '';
    const safe = /^(malformed|unsupported_version|invalid_cell|not_started|stunned|unavailable|finished|flagged|already_open|stale|command_limit|conflict|invalid_time|disconnected|invalid_epoch|invalid_sequence)$/.test(message) ? message : 'unavailable';
    scope.postMessage({ id: request.id, error: safe });
  }
}
scope.onmessage = event => { queue = queue.then(() => handle(event.data)); };
