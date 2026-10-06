import { describe, expect, it, vi } from 'vitest';
import { WorkerPracticeCore, type WorkerPort } from '@liar/core-bridge';
class FakeWorker extends EventTarget {
  sent: Array<{ id: number }> = [];
  terminate = vi.fn();
  postMessage(message: { id: number }) { this.sent.push(message); }
  reply(data: unknown) { this.dispatchEvent(new MessageEvent('message', { data })); }
}
describe('practice worker transport', () => {
  it('correlates out-of-order replies and ignores unknown requests', async () => {
    const worker = new FakeWorker(); const core = new WorkerPracticeCore(worker as unknown as WorkerPort);
    const first = core.snapshot(); const second = core.snapshot();
    const settled = Promise.all([first, second]);
    worker.reply({ id: 999, ok: { revision: 99 } });
    worker.reply({ id: worker.sent[1].id, ok: { revision: 2 } });
    worker.reply({ id: worker.sent[0].id, ok: { revision: 1 } });
    await expect(second).resolves.toEqual({ revision: 2 });
    await expect(first).resolves.toEqual({ revision: 1 });
    await settled;
    core.dispose();
  });
  it('bounds pending work and disposes all promises', async () => {
    const worker = new FakeWorker(); const core = new WorkerPracticeCore(worker as unknown as WorkerPort);
    const pending = Array.from({ length: 64 }, () => core.snapshot());
    const settled = Promise.allSettled(pending);
    await expect(core.snapshot()).rejects.toThrow('busy');
    core.dispose(); core.dispose();
    expect(worker.terminate).toHaveBeenCalledTimes(1);
    expect((await settled).every(result => result.status === 'rejected')).toBe(true);
    await expect(core.snapshot()).rejects.toThrow('disposed');
  });
  it('times out missing replies and propagates safe domain errors', async () => {
    vi.useFakeTimers();
    const worker = new FakeWorker(); const core = new WorkerPracticeCore(worker as unknown as WorkerPort, 100);
    const pending = core.snapshot(); const rejected = expect(pending).rejects.toThrow('timeout');
    await vi.advanceTimersByTimeAsync(100); await rejected;
    const next = core.snapshot(); worker.reply({ id: worker.sent[1].id, error: 'invalid_time' });
    await expect(next).rejects.toThrow('invalid_time');
    core.dispose(); vi.useRealTimers();
  });
});
