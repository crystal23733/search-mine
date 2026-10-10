// @vitest-environment node
import { expect, test, vi } from "vitest";
import { LatestResultController } from "./latest-result";
import { OnlineFailure } from "./types";
import type { PersonalResult } from "@liar/protocol";
import type { RequestOwner } from "../auth/requests";
import { personalResult, matchId } from "../../../test/online-fixture";
function setup() {
  let account: string | null = matchId,
    revision = 1,
    connected = true;
  const listeners = new Set<() => void>();
  const change = (id: string | null = account, online = connected) => {
    account = id;
    connected = online;
    revision++;
    for (const fn of listeners) fn();
  };
  const auth = {
    account: () => account,
    revision: () => revision,
    connected: () => connected && account !== null,
    subscribe: (fn: () => void) => {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
    invalidate: () => change(null, false),
  };
  const latestResult = vi.fn(
    async (
      _owner: RequestOwner,
      _signal: AbortSignal,
    ): Promise<PersonalResult | null> => personalResult(),
  );
  const controller = new LatestResultController(auth, { latestResult });
  return { controller, latestResult, change };
}
function deferred() {
  let resolve!: (result: PersonalResult | null) => void;
  const promise = new Promise<PersonalResult | null>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}
test("explicit discovery distinguishes known, absent, outage and unknown abort without joining a match", async () => {
  const s = setup();
  expect(s.latestResult).not.toHaveBeenCalled();
  await s.controller.start();
  expect(s.controller.read()).toEqual({
    status: "ready",
    result: personalResult(),
  });
  s.latestResult.mockResolvedValueOnce(null);
  await s.controller.start();
  expect(s.controller.read()).toEqual({ status: "empty", result: null });
  s.latestResult.mockRejectedValueOnce(new OnlineFailure("unavailable"));
  await s.controller.start();
  expect(s.controller.read()).toEqual({ status: "error", result: null });
  const unknown: PersonalResult = {
    ...personalResult(),
    own: null,
    end_elapsed_ms: null,
    result: { reason: "server_failure", outcome: "abort", completed: false },
  };
  s.latestResult.mockResolvedValueOnce(unknown);
  await s.controller.start();
  expect(s.controller.read()).toEqual({ status: "ready", result: unknown });
  s.controller.dispose();
});
test.each(["logout", "replace", "offline", "dispose"])(
  "%s cancels the read, hides data and ignores late replies",
  async (action) => {
    const s = setup(),
      late = deferred();
    s.latestResult.mockReturnValueOnce(late.promise);
    const pending = s.controller.start();
    expect(s.controller.read().status).toBe("loading");
    const signal = s.latestResult.mock.calls[0]![1];
    if (action === "dispose") s.controller.dispose();
    else
      s.change(
        action === "logout"
          ? null
          : action === "replace"
            ? "22222222-2222-4222-8222-222222222222"
            : matchId,
        action !== "offline",
      );
    expect(signal.aborted).toBe(true);
    expect(s.controller.read().result).toBeNull();
    late.resolve(personalResult());
    await pending;
    expect(s.controller.read().result).toBeNull();
    if (action === "logout" || action === "offline") {
      await s.controller.start();
      expect(s.latestResult).toHaveBeenCalledTimes(1);
    }
    s.controller.dispose();
  },
);
test("a fresh request owns the slot after an ignored older transport completes last", async () => {
  const s = setup(),
    old = deferred(),
    fresh = deferred();
  s.latestResult
    .mockReturnValueOnce(old.promise)
    .mockReturnValueOnce(fresh.promise);
  const a = s.controller.start(),
    b = s.controller.start();
  expect(s.latestResult.mock.calls[0]![1].aborted).toBe(true);
  fresh.resolve(null);
  await b;
  old.resolve(personalResult());
  await a;
  expect(s.controller.read()).toEqual({ status: "empty", result: null });
  s.controller.dispose();
});
test("lost authority clears an already published result and unauthorized never becomes empty", async () => {
  const s = setup();
  await s.controller.start();
  s.change(matchId, false);
  expect(s.controller.read().result).toBeNull();
  s.change(matchId, true);
  s.latestResult.mockRejectedValueOnce(new OnlineFailure("auth_required"));
  await s.controller.start();
  expect(s.controller.read()).toEqual({ status: "unauthorized", result: null });
  s.controller.dispose();
});
