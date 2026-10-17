import test from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { observeNativeWorkerTrace } from "../e2e/helpers/native-worker-trace.ts";
class Session extends EventEmitter {
  calls = [];
  detached = 0;
  constructor(mode = "sync") {
    super();
    this.mode = mode;
  }
  async send(method, params) {
    this.calls.push([method, params]);
    if (method === "Tracing.start" && this.mode === "busy")
      throw Error("private vendor data");
    if (method === "Tracing.end") {
      if (this.mode === "end-error") throw Error("private vendor data");
      if (this.mode === "sync") this.emit("Tracing.tracingComplete", {});
      if (this.mode === "hang")
        return new Promise((resolve) => {
          this.releaseEnd = resolve;
        });
    }
    return {};
  }
  async detach() {
    this.detached++;
  }
}
const listeners = (session) =>
  session.listenerCount("Tracing.dataCollected") +
  session.listenerCount("Tracing.tracingComplete");
test("native events and synchronous completion survive without worker/debugger commands", async () => {
  const session = new Session();
  const finish = await observeNativeWorkerTrace(session);
  const event = {
    name: "RequestTermination",
    args: { will_be_terminated: false },
  };
  session.emit("Tracing.dataCollected", { value: [event] });
  const result = await finish();
  assert.deepEqual(result, {
    events: [event],
    phase: "complete",
    complete: true,
    truncated: false,
    issue: null,
  });
  assert.deepEqual(session.calls, [
    [
      "Tracing.start",
      { categories: "ServiceWorker", transferMode: "ReportEvents" },
    ],
    ["Tracing.end", undefined],
  ]);
  assert.equal(session.detached, 1);
  assert.equal(listeners(session), 0);
});
test("a rejected start never terminates another owner's trace or exposes the exception", async () => {
  const session = new Session("busy");
  const finish = await observeNativeWorkerTrace(session);
  const result = await finish();
  assert.deepEqual(result, {
    events: [],
    complete: false,
    truncated: false,
    issue: "start_unavailable",
    phase: "not_started",
  });
  assert.equal(session.calls.length, 1);
  assert.equal(session.detached, 1);
  assert.equal(listeners(session), 0);
  assert.doesNotMatch(JSON.stringify(result), /private|vendor/);
});
test("end rejection preserves partial events and removes every observer", async () => {
  const session = new Session("end-error");
  const finish = await observeNativeWorkerTrace(session);
  session.emit("Tracing.dataCollected", { value: [{ name: "partial" }] });
  const result = await finish();
  assert.deepEqual(result.events, [{ name: "partial" }]);
  assert.equal(result.issue, "finish_unavailable");
  assert.equal(result.phase, "end_rejected");
  assert.equal(result.complete, false);
  assert.equal(session.detached, 1);
  assert.equal(listeners(session), 0);
});
test("a missing completion or hung End has a bounded finish and cleanup", async () => {
  for (const mode of ["missing", "hang"]) {
    const session = new Session(mode);
    const finish = await observeNativeWorkerTrace(session);
    const result = await finish();
    assert.equal(result.issue, "finish_unavailable");
    assert.equal(
      result.phase,
      mode === "missing" ? "completion_pending" : "end_pending",
    );
    assert.equal(result.complete, false);
    assert.equal(session.detached, 1);
    assert.equal(listeners(session), 0);
    const phase = result.phase;
    session.releaseEnd?.({});
    session.emit("Tracing.tracingComplete", {});
    session.emit("Tracing.dataCollected", { value: [{ name: "late" }] });
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(result.phase, phase);
    assert.deepEqual(result.events, []);
  }
});
test("concurrent repeated finish shares one End and one cleanup", async () => {
  const session = new Session();
  const finish = await observeNativeWorkerTrace(session);
  const one = finish();
  const two = finish();
  assert.equal(one, two);
  await one;
  assert.equal(
    session.calls.filter(([method]) => method === "Tracing.end").length,
    1,
  );
  assert.equal(session.detached, 1);
});
test("event memory is bounded and truncation is explicit", async () => {
  const session = new Session();
  const finish = await observeNativeWorkerTrace(session);
  session.emit("Tracing.dataCollected", {
    value: Array(50001).fill({ name: "event" }),
  });
  const result = await finish();
  assert.equal(result.events.length, 50000);
  assert.equal(result.truncated, true);
  assert.equal(result.complete, true);
});
