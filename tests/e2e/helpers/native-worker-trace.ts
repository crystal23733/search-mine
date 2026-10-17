import type { CDPSession } from "@playwright/test";
type TraceSession = Pick<CDPSession, "send" | "on" | "once" | "off" | "detach">;
export interface NativeWorkerTrace {
  events: unknown[];
  phase:
    | "not_started"
    | "recording"
    | "end_pending"
    | "end_rejected"
    | "completion_pending"
    | "complete";
  complete: boolean;
  truncated: boolean;
  issue: "start_unavailable" | "finish_unavailable" | null;
}
export async function observeNativeWorkerTrace(
  session: TraceSession,
): Promise<() => Promise<NativeWorkerTrace>> {
  const result: NativeWorkerTrace = {
    events: [],
    phase: "not_started",
    complete: false,
    truncated: false,
    issue: null,
  };
  const collect = ({ value }: { value: unknown[] }) => {
    const room = 50000 - result.events.length;
    result.events.push(...value.slice(0, room));
    result.truncated ||= value.length > room;
  };
  const detach = async () => {
    try {
      await session.detach();
    } catch {
      /* The browser may already be closed. */
    }
  };
  session.on("Tracing.dataCollected", collect);
  try {
    await session.send("Tracing.start", {
      categories: "ServiceWorker",
      transferMode: "ReportEvents",
    });
    result.phase = "recording";
  } catch {
    result.issue = "start_unavailable";
    session.off("Tracing.dataCollected", collect);
    await detach();
    const unavailable = Promise.resolve(result);
    return () => unavailable;
  }
  let finishing: Promise<NativeWorkerTrace> | undefined;
  return () =>
    (finishing ??= (async () => {
      let timer: ReturnType<typeof setTimeout> | undefined;
      let settled = false;
      let completed!: () => void;
      const completion = new Promise<void>((resolve) => {
        completed = resolve;
      });
      session.once("Tracing.tracingComplete", completed);
      result.phase = "end_pending";
      try {
        result.complete = await Promise.race([
          (async () => {
            await session.send("Tracing.end");
            if (settled) return false;
            result.phase = "completion_pending";
            await completion;
            return true;
          })().catch(() => {
            if (!settled) result.phase = "end_rejected";
            return false;
          }),
          new Promise<false>((resolve) => {
            timer = setTimeout(() => resolve(false), 5000);
          }),
        ]);
        settled = true;
        if (!result.complete) result.issue = "finish_unavailable";
        else result.phase = "complete";
        return result;
      } finally {
        settled = true;
        clearTimeout(timer);
        session.off("Tracing.tracingComplete", completed);
        session.off("Tracing.dataCollected", collect);
        await detach();
      }
    })());
}
