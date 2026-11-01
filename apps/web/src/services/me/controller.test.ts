// @vitest-environment node
import { expect, test, vi } from "vitest";
import { MeController } from "./controller";
import { MeFailure, type MePort } from "./types";
import { authTestPorts, TEST_ACCOUNT } from "../../../test/auth-ports";
import { overview, history, onlineEntry } from "../../../test/me-data";
async function fixture(mode: "overview" | "history" = "overview") {
  const auth = authTestPorts({ ...TEST_ACCOUNT, nickname: null }, true);
  await auth.auth.refresh();
  const port: MePort = {
    overview: vi.fn().mockResolvedValue(overview()),
    history: vi.fn().mockResolvedValue(history()),
  };
  return { ...auth, port, controller: new MeController(auth.auth, port, mode) };
}
test("self overview is memory-only, allows an unset nickname and clears immediately on invalidation", async () => {
  const f = await fixture();
  await f.controller.start();
  expect(f.controller.read().status).toBe("ready");
  expect(f.controller.read().overview?.account.id).toBe(TEST_ACCOUNT.id);
  f.auth.invalidate();
  expect(f.controller.read().overview).toBeNull();
  expect(f.controller.read().status).toBe("unauthorized");
  f.controller.dispose();
});
test("history uses opaque pages and full summaries, with filter changes cancelling and clearing old data", async () => {
  const f = await fixture("history");
  const first = {
    ...history(),
    total: 21,
    online: { ...history().online!, wins: 21, played: 21 },
    items: Array.from({ length: 20 }, (_, i) => ({
      kind: "online" as const,
      entry: onlineEntry(
        `c0000000-0000-4000-8000-${String(i + 1).padStart(12, "0")}`,
      ),
    })),
    next: "nextPage",
  };
  vi.mocked(f.port.history)
    .mockResolvedValueOnce(first)
    .mockResolvedValueOnce({
      ...first,
      items: [{ kind: "online", entry: onlineEntry() }],
      next: null,
    });
  await f.controller.start();
  expect(f.controller.read().history?.total).toBe(21);
  await f.controller.next();
  expect(f.controller.read().page).toBe(1);
  expect(vi.mocked(f.port.history).mock.calls[1][1].cursor).toBe("nextPage");
  vi.mocked(f.port.history).mockResolvedValueOnce(first);
  await f.controller.back();
  expect(f.controller.read().page).toBe(0);
  vi.mocked(f.port.history).mockResolvedValueOnce({
    ...history(),
    opponent: "bot",
    total: 0,
    items: [],
    online: {
      wins: 0,
      losses: 0,
      draws: 0,
      played: 0,
      aborted: 0,
      cancelled: 0,
      win_rate: null,
    },
  });
  await f.controller.filter("online", "bot");
  expect(f.controller.read().status).toBe("empty");
  expect(f.controller.read().history?.online?.win_rate).toBeNull();
  f.controller.dispose();
});
test("a late reply cannot restore an old account or overwrite a newer filter", async () => {
  const f = await fixture();
  let reply!: (value: ReturnType<typeof overview>) => void;
  vi.mocked(f.port.overview).mockImplementationOnce(
    () =>
      new Promise((r) => {
        reply = r;
      }),
  );
  const pending = f.controller.start();
  await vi.waitFor(() => expect(f.port.overview).toHaveBeenCalled());
  f.auth.suspend();
  reply(overview());
  await pending;
  expect(f.controller.read().overview).toBeNull();
  expect(f.controller.read().status).toBe("unauthorized");
  f.controller.dispose();
  const g = await fixture("history");
  let old!: (v: ReturnType<typeof history>) => void;
  vi.mocked(g.port.history).mockImplementationOnce(
    () =>
      new Promise((r) => {
        old = r;
      }),
  );
  const previous = g.controller.start();
  await vi.waitFor(() => expect(g.port.history).toHaveBeenCalled());
  vi.mocked(g.port.history).mockResolvedValueOnce({
    ...history(),
    opponent: "bot",
    total: 0,
    items: [],
  });
  await g.controller.filter("online", "bot");
  old(history());
  await previous;
  expect(g.controller.read().opponent).toBe("bot");
  expect(g.controller.read().history?.items).toEqual([]);
  g.controller.dispose();
});
test("errors and corrupt owner/filter replies are not represented as empty successful histories", async () => {
  const f = await fixture("history");
  vi.mocked(f.port.history).mockRejectedValueOnce(
    new MeFailure("rate_limited"),
  );
  await f.controller.start();
  expect(f.controller.read().status).toBe("error");
  expect(f.controller.read().error).toBe("rate_limited");
  vi.mocked(f.port.history).mockResolvedValueOnce({
    ...history(),
    account_id: "a0000000-0000-4000-8000-000000000002",
  });
  await f.controller.start();
  expect(f.controller.read().status).toBe("error");
  expect(f.controller.read().history).toBeNull();
  f.controller.dispose();
});

test("an in-flight change clears the private snapshot and reloads after the same account finishes", async () => {
  const f = await fixture();
  await f.controller.start();
  let finish!: (value: typeof TEST_ACCOUNT) => void;
  vi.mocked(f.authTransport.nickname).mockImplementationOnce(
    () =>
      new Promise((r) => {
        finish = r;
      }),
  );
  const mutation = f.auth.nickname("Renamed", TEST_ACCOUNT.id);
  await vi.waitFor(() => expect(f.authTransport.nickname).toHaveBeenCalled());
  expect(f.controller.read().overview).toBeNull();
  expect(f.controller.read().status).toBe("loading");
  vi.mocked(f.port.overview).mockResolvedValueOnce({
    ...overview(),
    account: { ...overview().account, nickname: "Renamed" },
  });
  finish({ ...TEST_ACCOUNT, nickname: "Renamed" });
  await mutation;
  await vi.waitFor(() =>
    expect(f.controller.read().overview?.account.nickname).toBe("Renamed"),
  );
  f.controller.dispose();
});
