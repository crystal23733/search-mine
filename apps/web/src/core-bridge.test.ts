import { describe, expect, it, vi } from "vitest";
import {
  WorkerPracticeCore,
  WorkerTrainingCore,
  WorkerDailyCore,
  type WorkerPort,
} from "@liar/core-bridge";
class FakeWorker extends EventTarget {
  sent: Array<{ id: number }> = [];
  terminate = vi.fn();
  postMessage(message: { id: number }) {
    this.sent.push(message);
  }
  reply(data: unknown) {
    this.dispatchEvent(new MessageEvent("message", { data }));
  }
}
describe("practice worker transport", () => {
  it("binds daily initialization to the UTC date and retrieves its native replay", async () => {
    const worker = new FakeWorker();
    const core = new WorkerDailyCore(worker as unknown as WorkerPort);
    const initial = core.init("2026-10-07", 1);
    const check = expect(initial).resolves.toEqual({
      metadata: { date: "2026-10-07" },
    });
    worker.reply({ id: 1, ok: { metadata: { date: "2026-10-07" } } });
    await check;
    expect(worker.sent[0]).toEqual({
      id: 1,
      kind: "daily-init",
      date: "2026-10-07",
      seed_version: 1,
    });
    const replay = core.replay();
    worker.reply({ id: 2, ok: { v: 1, inputs: [], final_time_ms: 0 } });
    await expect(replay).resolves.toMatchObject({
      inputs: [],
      final_time_ms: 0,
    });
    expect(worker.sent[1]).toEqual({ id: 2, kind: "daily-replay" });
    core.dispose();
  });
  it("starts training in its separate worker mode and correlates lesson replies", async () => {
    const worker = new FakeWorker();
    const core = new WorkerTrainingCore(worker as unknown as WorkerPort);
    const initial = core.init();
    const initialCheck = expect(initial).resolves.toEqual({ stage: "open" });
    worker.reply({ id: worker.sent[0].id, ok: { stage: "open" } });
    await initialCheck;
    expect(worker.sent[0]).toEqual({ id: 1, kind: "training-init" });
    const snapshot = core.snapshot();
    worker.reply({ id: worker.sent[1].id, ok: { stage: "attack" } });
    await expect(snapshot).resolves.toEqual({ stage: "attack" });
    const input = {
      v: 1,
      command_id: 1,
      client_seq: 1,
      action: { type: "attack" },
    } as const;
    const step = core.step(input, 1);
    worker.reply({
      id: worker.sent[2].id,
      ok: { ack: { duplicate: false }, view: { stage: "reveal" } },
    });
    await expect(step).resolves.toMatchObject({ view: { stage: "reveal" } });
    expect(worker.sent[2]).toEqual({
      id: 3,
      kind: "training-step",
      input,
      time_ms: 1,
    });
    const tick = core.advance(5);
    worker.reply({ id: worker.sent[3].id, ok: { stage: "accuse" } });
    await expect(tick).resolves.toEqual({ stage: "accuse" });
    core.dispose();
  });
  it("correlates out-of-order replies and ignores unknown requests", async () => {
    const worker = new FakeWorker();
    const core = new WorkerPracticeCore(worker as unknown as WorkerPort);
    const first = core.snapshot();
    const second = core.snapshot();
    const settled = Promise.all([first, second]);
    worker.reply({ id: 999, ok: { revision: 99 } });
    worker.reply({ id: worker.sent[1].id, ok: { revision: 2 } });
    worker.reply({ id: worker.sent[0].id, ok: { revision: 1 } });
    await expect(second).resolves.toEqual({ revision: 2 });
    await expect(first).resolves.toEqual({ revision: 1 });
    await settled;
    core.dispose();
  });
  it("bounds pending work and disposes all promises", async () => {
    const worker = new FakeWorker();
    const core = new WorkerPracticeCore(worker as unknown as WorkerPort);
    const pending = Array.from({ length: 64 }, () => core.snapshot());
    const settled = Promise.allSettled(pending);
    await expect(core.snapshot()).rejects.toThrow("busy");
    core.dispose();
    core.dispose();
    expect(worker.terminate).toHaveBeenCalledTimes(1);
    expect(
      (await settled).every((result) => result.status === "rejected"),
    ).toBe(true);
    await expect(core.snapshot()).rejects.toThrow("disposed");
  });
  it("times out missing replies and propagates safe domain errors", async () => {
    vi.useFakeTimers();
    const worker = new FakeWorker();
    const core = new WorkerPracticeCore(worker as unknown as WorkerPort, 100);
    const pending = core.snapshot();
    const rejected = expect(pending).rejects.toThrow("timeout");
    await vi.advanceTimersByTimeAsync(100);
    await rejected;
    const next = core.snapshot();
    worker.reply({ id: worker.sent[1].id, error: "invalid_time" });
    await expect(next).rejects.toThrow("invalid_time");
    core.dispose();
    vi.useRealTimers();
  });
});
