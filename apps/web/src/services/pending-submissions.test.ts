import { expect, test, vi } from "vitest";
import { createDailyRecords } from "./daily-records";
import { createPendingSubmissions } from "./pending-submissions";
import { record } from "../../test/daily-record";
test("only explicit current account and connectivity can submit, retries reuse attempt and never claim verified local time", async () => {
  const records = createDailyRecords();
  await records.saveFirst(record());
  let account: string | null = null,
    connected = true;
  const submit = vi
    .fn()
    .mockRejectedValueOnce(Error("offline"))
    .mockResolvedValue("accepted");
  const queue = createPendingSubmissions(
    records,
    {
      account: () => account,
      revision: () => 0,
      connected: () => connected,
      subscribe: () => () => {},
    },
    { submit },
  );
  await queue.flush("A");
  account = "A";
  connected = false;
  await queue.flush("A");
  connected = true;
  await queue.flush("B");
  expect(submit).not.toHaveBeenCalled();
  expect((await queue.flush("A")).retained).toBe(true);
  expect((await records.pending()).value).toHaveLength(1);
  expect(await queue.flush("A")).toEqual({
    sent: 1,
    removed: 1,
    retained: false,
  });
  expect(submit.mock.calls.map((call) => call[0].attempt_id)).toEqual([
    "first",
    "first",
  ]);
  expect((await records.list()).value).toEqual([record()]);
});
test("returning to the same account after a session revision or a hanging transport never deletes the candidate", async () => {
  const records = createDailyRecords();
  await records.saveFirst(record());
  let revision = 1,
    resolve!: (value: "accepted") => void;
  const submit = vi.fn(
    (_record, _account, _signal: AbortSignal) =>
      new Promise<"accepted">((reply) => {
        resolve = reply;
      }),
  );
  const queue = createPendingSubmissions(
    records,
    {
      account: () => "A",
      revision: () => revision,
      connected: () => true,
      subscribe: () => () => {},
    },
    { submit },
  );
  const old = queue.flush("A");
  await vi.waitFor(() => expect(submit).toHaveBeenCalledTimes(1));
  revision++;
  resolve("accepted");
  expect((await old).retained).toBe(true);
  vi.useFakeTimers();
  try {
    const hanging = queue.flush("A");
    await vi.advanceTimersByTimeAsync(10_000);
    expect((await hanging).retained).toBe(true);
    expect(submit.mock.calls[1][2].aborted).toBe(true);
    resolve("accepted");
    await Promise.resolve();
    expect((await records.pending()).value).toEqual([record()]);
  } finally {
    vi.useRealTimers();
  }
});
test("coalesces one account flush and account changes or network loss keep candidates; expired is not a verified completion", async () => {
  const records = createDailyRecords();
  await records.saveFirst(record());
  let account = "A";
  let reply!: (value: "accepted" | "expired") => void;
  const submit = vi.fn(
    () =>
      new Promise<"accepted" | "expired">((resolve) => {
        reply = resolve;
      }),
  );
  const queue = createPendingSubmissions(
    records,
    {
      account: () => account,
      revision: () => 0,
      connected: () => true,
      subscribe: () => () => {},
    },
    { submit },
  );
  const first = queue.flush("A"),
    duplicate = queue.flush("A");
  expect(duplicate).toBe(first);
  await vi.waitFor(() => expect(submit).toHaveBeenCalledTimes(1));
  account = "B";
  reply("accepted");
  expect((await first).retained).toBe(true);
  expect((await records.pending()).value).toHaveLength(1);
  const next = queue.flush("B");
  await vi.waitFor(() => expect(submit).toHaveBeenCalledTimes(2));
  reply("expired");
  expect(await next).toEqual({ sent: 0, removed: 1, retained: false });
  expect((await records.pending()).value).toEqual([]);
  expect((await records.list()).value[0].status).toBe("unverified");
});
