import {
  DEFAULT_RULES,
  DAILY_SEED_VERSION,
  type DailyMetadata,
  type DailyReplay,
} from "@liar/protocol";
export interface DailyRecord {
  v: 1;
  id: string;
  status: "unverified";
  attempt_id: string;
  metadata: DailyMetadata;
  elapsed_ms: number;
  mistakes: number;
  replay: DailyReplay;
}
export type RecordsResult<T> = { value: T; warning: boolean };
export interface DailyRecordsPort {
  list(): Promise<RecordsResult<DailyRecord[]>>;
  saveFirst(
    record: DailyRecord,
  ): Promise<RecordsResult<{ record: DailyRecord; inserted: boolean }>>;
  clear(): Promise<RecordsResult<null>>;
  pending(): Promise<RecordsResult<DailyRecord[]>>;
  removePending(id: string, attemptId: string): Promise<RecordsResult<null>>;
}
export function dailyId(metadata: DailyMetadata): string {
  return `${metadata.date}:v${metadata.seed_version}:${metadata.rules_hash}`;
}
function shape(
  value: unknown,
  keys: string[],
): value is Record<string, unknown> {
  return (
    typeof value === "object" &&
    value !== null &&
    !Array.isArray(value) &&
    Object.keys(value).length === keys.length &&
    keys.every((key) => Object.hasOwn(value, key))
  );
}
function uint(value: unknown): value is number {
  return (
    Number.isInteger(value) &&
    (value as number) >= 0 &&
    (value as number) <= 0xffffffff
  );
}
function metadata(value: unknown): value is DailyMetadata {
  return (
    shape(value, [
      "date",
      "seed_version",
      "mode",
      "seed",
      "rules_hash",
      "solver_version",
      "rng_version",
    ]) &&
    typeof value.date === "string" &&
    /^\d{4}-\d{2}-\d{2}$/.test(value.date) &&
    value.seed_version === DAILY_SEED_VERSION &&
    value.mode === "solo-v1" &&
    typeof value.seed === "string" &&
    /^\d{1,20}$/.test(value.seed) &&
    typeof value.rules_hash === "string" &&
    value.rules_hash.length > 0 &&
    value.rules_hash.length <= 128 &&
    uint(value.solver_version) &&
    uint(value.rng_version)
  );
}
function parseRecord(value: unknown): DailyRecord | null {
  try {
    if (
      new TextEncoder().encode(JSON.stringify(value)).byteLength >
        2 * 1024 * 1024 ||
      !shape(value, [
        "v",
        "id",
        "status",
        "attempt_id",
        "metadata",
        "elapsed_ms",
        "mistakes",
        "replay",
      ]) ||
      value.v !== 1 ||
      value.status !== "unverified" ||
      !metadata(value.metadata) ||
      value.id !== dailyId(value.metadata) ||
      typeof value.attempt_id !== "string" ||
      value.attempt_id.length < 1 ||
      value.attempt_id.length > 128 ||
      !uint(value.elapsed_ms) ||
      !uint(value.mistakes)
    )
      return null;
    const replay = value.replay;
    if (
      !shape(replay, ["v", "metadata", "inputs", "final_time_ms"]) ||
      replay.v !== 1 ||
      !metadata(replay.metadata) ||
      !sameMetadata(value.metadata, replay.metadata) ||
      !uint(replay.final_time_ms) ||
      !Array.isArray(replay.inputs) ||
      replay.inputs.length > DEFAULT_RULES.rules.max_commands_per_seat
    )
      return null;
    for (const item of replay.inputs) {
      if (
        !shape(item, ["input", "time_ms"]) ||
        !uint(item.time_ms) ||
        !shape(item.input, ["v", "command_id", "client_seq", "action"]) ||
        item.input.v !== 1 ||
        !uint(item.input.command_id) ||
        item.input.command_id === 0 ||
        !uint(item.input.client_seq) ||
        item.input.client_seq === 0
      )
        return null;
      const action = item.input.action;
      if (
        !(shape(action, ["type"]) && action.type === "attack") &&
        !(
          shape(action, ["type", "cell"]) &&
          ["open", "flag", "accuse"].includes(action.type as string) &&
          uint(action.cell)
        )
      )
        return null;
    }
    return JSON.parse(JSON.stringify(value)) as DailyRecord;
  } catch {
    return null;
  }
}
function sameMetadata(first: DailyMetadata, second: DailyMetadata): boolean {
  return Object.keys(first).every(
    (key) =>
      first[key as keyof DailyMetadata] === second[key as keyof DailyMetadata],
  );
}
export function createDailyRecords(
  factory?: IDBFactory,
  name = "liar.daily.v1",
): DailyRecordsPort {
  const memory = new Map<string, DailyRecord>();
  const queued = new Map<string, DailyRecord>();
  let fallback = !factory;
  let warning = fallback;
  const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T;
  const ordered = (source = memory) =>
    [...source.values()].sort((a, b) => b.id.localeCompare(a.id));
  const remember = (record: DailyRecord) => {
    memory.set(record.id, record);
    for (const stale of ordered().slice(30)) {
      memory.delete(stale.id);
      queued.delete(stale.id);
    }
  };
  const open = () =>
    new Promise<IDBDatabase>((resolve, reject) => {
      const request = factory!.open(name, 2);
      let done = false;
      const fail = () => {
        if (!done) {
          done = true;
          clearTimeout(timer);
          reject(Error("storage_unavailable"));
        }
      };
      const timer = setTimeout(fail, 5000);
      request.onerror = fail;
      request.onblocked = fail;
      request.onupgradeneeded = () => {
        const db = request.result;
        if (!db.objectStoreNames.contains("records"))
          db.createObjectStore("records", { keyPath: "id" });
        const pending = db.createObjectStore("pending", { keyPath: "id" });
        const cursor = request
          .transaction!.objectStore("records")
          .openCursor(null, "prev");
        let visited = 0;
        cursor.onsuccess = () => {
          const row = cursor.result;
          if (!row || visited++ >= 30) return;
          const record = parseRecord(row.value);
          if (record) pending.put(record);
          else warning = true;
          row.continue();
        };
      };
      request.onsuccess = () => {
        if (done) {
          request.result.close();
          return;
        }
        done = true;
        clearTimeout(timer);
        request.result.onversionchange = () => request.result.close();
        resolve(request.result);
      };
    });
  const run = async <T>(
    mode: IDBTransactionMode,
    operation: (
      store: IDBObjectStore,
      set: (value: T) => void,
      pending: IDBObjectStore,
    ) => void,
  ): Promise<T> => {
    const db = await open();
    try {
      return await new Promise<T>((resolve, reject) => {
        const tx = db.transaction(["records", "pending"], mode);
        let value: T;
        const timer = setTimeout(() => {
          tx.abort();
          reject(Error("storage_unavailable"));
        }, 5000);
        tx.oncomplete = () => {
          clearTimeout(timer);
          resolve(value);
        };
        tx.onabort = tx.onerror = () => {
          clearTimeout(timer);
          reject(Error("storage_unavailable"));
        };
        operation(
          tx.objectStore("records"),
          (result) => {
            value = result;
          },
          tx.objectStore("pending"),
        );
      });
    } finally {
      db.close();
    }
  };
  const failed = () => {
    fallback = true;
    warning = true;
  };
  const read = async (
    pending: boolean,
  ): Promise<RecordsResult<DailyRecord[]>> => {
    const target = pending ? queued : memory;
    if (!fallback)
      try {
        const rows = await run<DailyRecord[]>(
          "readonly",
          (store, set, queue) => {
            const result: DailyRecord[] = [];
            const cursor = (pending ? queue : store).openCursor(null, "prev");
            let visited = 0;
            cursor.onsuccess = () => {
              const row = cursor.result;
              if (!row || visited++ >= 30) {
                set(result);
                return;
              }
              const parsed = parseRecord(row.value);
              if (parsed) result.push(parsed);
              else warning = true;
              row.continue();
            };
          },
        );
        target.clear();
        rows.forEach((row) => target.set(row.id, row));
      } catch {
        failed();
      }
    return { value: copy(ordered(target)), warning };
  };
  return {
    list: () => read(false),
    pending: () => read(true),
    async removePending(id, attemptId) {
      if (!fallback)
        try {
          await run<null>("readwrite", (_store, set, pending) => {
            const request = pending.get(id);
            request.onsuccess = () => {
              if (parseRecord(request.result)?.attempt_id === attemptId)
                pending.delete(id);
              set(null);
            };
          });
        } catch {
          failed();
        }
      if (queued.get(id)?.attempt_id === attemptId) queued.delete(id);
      return { value: null, warning };
    },
    async saveFirst(source) {
      const record = parseRecord(source);
      if (!record) throw Error("malformed_record");
      if (!fallback)
        try {
          const saved = await run<{ record: DailyRecord; inserted: boolean }>(
            "readwrite",
            (store, set, pending) => {
              const request = store.get(record.id);
              request.onsuccess = () => {
                const previous = parseRecord(request.result);
                if (previous) {
                  set({ record: previous, inserted: false });
                  return;
                }
                if (request.result !== undefined) warning = true;
                store.put(record);
                pending.put(record);
                set({ record, inserted: true });
                const count = store.count();
                count.onsuccess = () => {
                  let excess = Math.max(0, count.result - 30);
                  if (!excess) return;
                  const cursor = store.openCursor();
                  cursor.onsuccess = () => {
                    const row = cursor.result;
                    if (row && excess-- > 0) {
                      row.delete();
                      pending.delete(row.primaryKey);
                      row.continue();
                    }
                  };
                };
              };
            },
          );
          if (saved.inserted) queued.set(saved.record.id, saved.record);
          remember(saved.record);
          return { value: copy(saved), warning };
        } catch {
          failed();
        }
      const previous = memory.get(record.id);
      if (!previous) queued.set(record.id, record);
      remember(previous ?? record);
      return {
        value: copy({ record: previous ?? record, inserted: !previous }),
        warning: true,
      };
    },
    async clear() {
      if (!fallback)
        try {
          await run<null>("readwrite", (store, set, pending) => {
            store.clear();
            pending.clear();
            set(null);
          });
        } catch {
          failed();
        }
      memory.clear();
      queued.clear();
      return { value: null, warning };
    },
  };
}
