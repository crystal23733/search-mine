import type { CoreReply, CoreRequest } from './index';
import type { GameView, LocalStep, TutorialView, TutorialStep } from '@liar/protocol';
import type { LocalSession, TrainingSession } from './wasm-bindings';
type Request = CoreRequest & { id: number };
const scope = self as unknown as { onmessage: ((event: MessageEvent<Request>) => void) | null; postMessage: (reply: CoreReply) => void };
let session: LocalSession | TrainingSession | undefined;
let training = false;
let queue = Promise.resolve();
async function handle(request: Request) {
  try {
    if (!Number.isSafeInteger(request.id) || request.id <= 0) return;
    const isTraining = request.kind.startsWith('training-');
    if (request.kind === 'init' || request.kind === 'training-init') {
      if (session) throw new Error('unavailable');
      const uri = new URL('/core/liar_wasm.js', self.location.origin).href;
      const wasm = await import(/* @vite-ignore */ uri) as typeof import('./wasm-bindings');
      await wasm.default();
      training = isTraining;
      session = request.kind === 'training-init' ? new wasm.TrainingSession() : new wasm.LocalSession(request.seed, request.difficulty);
    }
    if (!session) throw new Error('not_started');
    if (isTraining !== training) throw new Error('unavailable');
    if ('time_ms' in request) {
      if (!Number.isInteger(request.time_ms) || request.time_ms < 0 || request.time_ms > 0xffffffff) throw new Error('invalid_time');
    }
    const value = 'input' in request ? session.step(JSON.stringify(request.input), request.time_ms) : 'time_ms' in request ? session.advance(request.time_ms) : session.snapshot();
    scope.postMessage({ id: request.id, ok: JSON.parse(value) as GameView | LocalStep | TutorialView | TutorialStep });
  } catch (error) {
    // Rust adapter errors contain only stable public error codes. Other JS failures are generic.
    const message = error instanceof Error ? error.message : '';
    const safe = /^(malformed|unsupported_version|invalid_cell|not_started|stunned|unavailable|finished|flagged|already_open|stale|command_limit|conflict|invalid_time|disconnected|invalid_epoch|invalid_sequence)$/.test(message) ? message : 'unavailable';
    scope.postMessage({ id: request.id, error: safe });
  }
}
scope.onmessage = event => { queue = queue.then(() => handle(event.data)); };
