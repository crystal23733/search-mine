import type { DailyRecord, DailyRecordsPort } from "./daily-records";
export interface SubmissionSession {
  account(): string | null;
  revision(): number;
  connected(): boolean;
}
export interface DailySubmissionTransport {
  submit(
    record: DailyRecord,
    accountId: string,
    signal: AbortSignal,
  ): Promise<"accepted" | "expired" | "unsupported">;
}
export type SubmissionResult = {
  sent: number;
  removed: number;
  retained: boolean;
};
export interface PendingSubmissionsPort {
  flush(confirmedAccount: string): Promise<SubmissionResult>;
}
export function createPendingSubmissions(
  records: DailyRecordsPort,
  session: SubmissionSession,
  transport: DailySubmissionTransport,
): PendingSubmissionsPort {
  let running:
    { account: string; promise: Promise<SubmissionResult> } | undefined;
  const execute = async (account: string): Promise<SubmissionResult> => {
    const revision = session.revision();
    const allowed = () =>
      account.length > 0 &&
      session.account() === account &&
      session.revision() === revision &&
      session.connected();
    const result: SubmissionResult = { sent: 0, removed: 0, retained: true };
    if (!allowed()) return result;
    const candidates = await records.pending();
    for (const record of candidates.value) {
      if (!allowed()) return result;
      const controller = new AbortController();
      let timer: ReturnType<typeof setTimeout> | undefined;
      try {
        const timeout = new Promise<never>((_resolve, reject) => {
          timer = setTimeout(() => {
            controller.abort();
            reject(Error("timeout"));
          }, 10_000);
        });
        const reply = await Promise.race([
          transport.submit(record, account, controller.signal),
          timeout,
        ]);
        if (
          !allowed() ||
          !["accepted", "expired", "unsupported"].includes(reply)
        )
          return result;
        await records.removePending(record.id, record.attempt_id);
        result.removed++;
        if (reply === "accepted") result.sent++;
      } catch {
        return result;
      } finally {
        clearTimeout(timer);
      }
    }
    result.retained = (await records.pending()).value.length > 0;
    return result;
  };
  return {
    flush(account) {
      if (running)
        return running.account === account
          ? running.promise
          : Promise.resolve({ sent: 0, removed: 0, retained: true });
      const promise = execute(account)
        .catch(() => ({ sent: 0, removed: 0, retained: true }))
        .finally(() => {
          running = undefined;
        });
      running = { account, promise };
      return promise;
    },
  };
}
