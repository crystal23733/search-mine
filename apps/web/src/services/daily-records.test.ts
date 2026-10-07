import { expect, test, vi } from "vitest";
import { IDBFactory } from "fake-indexeddb";
import { record } from "../../test/daily-record";
import { createDailyRecords, type DailyRecord } from "./daily-records";
test("two repositories atomically keep the first local clear and reload without claiming server verification", async () => {
  const db = new IDBFactory();
  const first = createDailyRecords(db),
    second = createDailyRecords(db);
  const saved = await Promise.all([
    first.saveFirst(record()),
    second.saveFirst(record("2026-10-07", "retry")),
  ]);
  expect(saved.filter((result) => result.value.inserted)).toHaveLength(1);
  expect(saved.every((result) => !result.warning)).toBe(true);
  const loaded = await createDailyRecords(db).list();
  expect(loaded.value).toHaveLength(1);
  expect(loaded.value[0].attempt_id).toBe(saved[0].value.record.attempt_id);
  expect(loaded.value[0].status).toBe("unverified");
  await first.clear();
  expect((await second.list()).value).toEqual([]);
});
test("keeps at most 30 dates and missing or failed storage falls back to this tab's memory", async () => {
  const persistent = createDailyRecords(new IDBFactory());
  for (let day = 1; day <= 31; day++)
    await persistent.saveFirst(
      record(`2026-10-${String(day).padStart(2, "0")}`),
    );
  const loaded = await persistent.list();
  expect(loaded.value).toHaveLength(30);
  expect((await persistent.pending()).value).toHaveLength(30);
  expect(
    loaded.value.some((value) => value.metadata.date === "2026-10-01"),
  ).toBe(false);
  for (const factory of [
    undefined,
    {
      open: () => {
        throw Error("denied");
      },
    } as unknown as IDBFactory,
  ]) {
    const memory = createDailyRecords(factory);
    expect((await memory.saveFirst(record())).warning).toBe(true);
    expect(
      (await memory.saveFirst(record("2026-10-07", "retry"))).value.inserted,
    ).toBe(false);
    expect((await memory.list()).value[0].attempt_id).toBe("first");
  }
});
test("rejects foreign or oversized stored data instead of treating it as a verified result", async () => {
  const records = createDailyRecords();
  await expect(
    records.saveFirst({
      ...record(),
      status: "verified",
    } as unknown as DailyRecord),
  ).rejects.toThrow("malformed_record");
  await expect(
    records.saveFirst({ ...record(), attempt_id: "x".repeat(3_000_000) }),
  ).rejects.toThrow("malformed_record");
});
test("ignores corrupt database rows with a warning and a stalled open has a bounded memory fallback", async () => {
  const factory = new IDBFactory();
  const records = createDailyRecords(factory);
  await records.saveFirst(record());
  const request = factory.open("liar.daily.v1");
  const db = await new Promise<IDBDatabase>((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = reject;
  });
  await new Promise<void>((resolve, reject) => {
    const tx = db.transaction("records", "readwrite");
    tx.objectStore("records").put({
      id: "2026-10-08:v1:hash",
      status: "verified",
      nickname: "foreign",
    });
    tx.oncomplete = () => resolve();
    tx.onabort = reject;
  });
  db.close();
  const result = await records.list();
  expect(result.warning).toBe(true);
  expect(result.value).toEqual([record()]);
  vi.useFakeTimers();
  try {
    const stalled = createDailyRecords({
      open: () => ({}),
    } as unknown as IDBFactory);
    const save = stalled.saveFirst(record());
    await vi.advanceTimersByTimeAsync(5000);
    expect((await save).warning).toBe(true);
    expect((await stalled.list()).value).toEqual([record()]);
  } finally {
    vi.useRealTimers();
  }
});
