import { expect, test, vi } from "vitest";
import { IDBFactory, IDBObjectStore } from "fake-indexeddb";
import { record } from "../../test/daily-record";
import { createDailyRecords } from "./daily-records";
test("first completion and its pending replay are atomic, acknowledging a retry cannot remove another attempt", async () => {
  for (const factory of [new IDBFactory(), undefined]) {
    const repository = createDailyRecords(factory);
    await repository.saveFirst(record());
    expect((await repository.pending()).value).toEqual([record()]);
    await repository.saveFirst(record("2026-10-07", "practice"));
    await repository.removePending(record().id, "foreign");
    expect((await repository.pending()).value).toEqual([record()]);
    await repository.removePending(record().id, "first");
    expect((await repository.pending()).value).toEqual([]);
    expect((await repository.list()).value).toEqual([record()]);
    await repository.saveFirst(record("2026-10-07", "after-ack"));
    expect((await repository.pending()).value).toEqual([]);
    await repository.saveFirst(record("2026-10-08"));
    await repository.clear();
    expect((await repository.pending()).value).toEqual([]);
    expect((await repository.list()).value).toEqual([]);
  }
});
test("a quota abort while writing pending leaves neither store partially committed and falls back to memory", async () => {
  const factory = new IDBFactory(),
    repository = createDailyRecords(factory);
  const put = IDBObjectStore.prototype.put;
  const quota = vi
    .spyOn(IDBObjectStore.prototype, "put")
    .mockImplementation(function (this: IDBObjectStore, value, key) {
      const request = put.call(this, value, key);
      if (this.name === "pending") this.transaction.abort();
      return request;
    });
  try {
    expect((await repository.saveFirst(record())).warning).toBe(true);
    expect((await repository.pending()).value).toEqual([record()]);
  } finally {
    quota.mockRestore();
  }
  const loaded = createDailyRecords(factory);
  expect((await loaded.list()).value).toEqual([]);
  expect((await loaded.pending()).value).toEqual([]);
});
test("upgrading existing v1 local records preserves personal results and adds bounded unverified candidates", async () => {
  const factory = new IDBFactory();
  const request = factory.open("liar.daily.v1", 1);
  request.onupgradeneeded = () =>
    request.result.createObjectStore("records", { keyPath: "id" });
  const db = await new Promise<IDBDatabase>((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = reject;
  });
  await new Promise<void>((resolve, reject) => {
    const tx = db.transaction("records", "readwrite");
    tx.objectStore("records").put(record());
    tx.oncomplete = () => resolve();
    tx.onabort = reject;
  });
  db.close();
  const repository = createDailyRecords(factory);
  expect((await repository.pending()).value).toEqual([record()]);
  expect((await repository.list()).value).toEqual([record()]);
});
