// @vitest-environment node
import { expect, test, vi } from "vitest";
import { authTestPorts, TEST_ACCOUNT } from "../../../test/auth-ports";
import type { ConsentSnapshot } from "@liar/protocol";
import { ConsentController } from "./controller";
import { PrivacyFailure } from "./types";
function value(): ConsentSnapshot {
  return {
    v: 1,
    account_id: TEST_ACCOUNT.id,
    revision: null,
    observed_at: 1500,
    consent: {
      cmp_available: false,
      status: "unknown",
      analytics: "unknown",
      advertising: "unknown",
      checked_at: null,
      expires_at: null,
    },
  };
}
async function fixture() {
  const f = authTestPorts({ ...TEST_ACCOUNT, nickname: null }, true);
  await f.auth.refresh();
  const api = {
    read: vi.fn().mockResolvedValue(value()),
    refresh: vi.fn().mockResolvedValue(value()),
    withdraw: vi.fn().mockResolvedValue(value()),
  };
  return {
    ...f,
    api,
    controller: new ConsentController(f.auth, api, () => 100),
  };
}
test("nickname-free controls load current state and use the last server CAS revision", async () => {
  const f = await fixture();
  expect(await f.controller.load()).toBe(true);
  expect(f.controller.read()).toMatchObject({
    status: "ready",
    snapshot: value(),
    startedAt: 100,
  });
  await f.controller.refresh();
  expect(f.api.refresh.mock.calls[0][1]).toBeNull();
  await f.controller.withdraw("analytics");
  expect(f.api.withdraw.mock.calls[0][1]).toBe("analytics");
  f.controller.dispose();
});
test("starting a mutation removes the old view and an unavailable response does not restore it", async () => {
  const f = await fixture();
  await f.controller.load();
  let reject!: (error: Error) => void;
  f.api.withdraw.mockImplementation(
    () =>
      new Promise((_r, j) => {
        reject = j;
      }),
  );
  const pending = f.controller.withdraw("all");
  expect(f.controller.read()).toMatchObject({
    status: "loading",
    snapshot: null,
  });
  reject(new PrivacyFailure("conflict"));
  expect(await pending).toBe(false);
  expect(f.controller.read()).toMatchObject({
    status: "error",
    snapshot: null,
    error: "conflict",
  });
  f.controller.dispose();
});
test("logout and disposal abort pending self requests and ignore late responses", async () => {
  for (const logout of [true, false]) {
    const f = await fixture();
    let resolve!: (v: ConsentSnapshot) => void;
    f.api.read.mockImplementation(
      () =>
        new Promise((r) => {
          resolve = r;
        }),
    );
    const pending = f.controller.load();
    const signal = f.api.read.mock.calls[0][1] as AbortSignal;
    if (logout) f.auth.invalidate();
    else f.controller.dispose();
    expect(signal.aborted).toBe(true);
    resolve(value());
    expect(await pending).toBe(false);
    expect(f.controller.read().snapshot).toBeNull();
    f.controller.dispose();
  }
});
test("manual cancel allows a new read but an old ignored completion cannot replace it", async () => {
  const f = await fixture();
  let resolve!: (v: ConsentSnapshot) => void;
  f.api.read.mockImplementationOnce(
    () =>
      new Promise((r) => {
        resolve = r;
      }),
  );
  const pending = f.controller.load();
  f.controller.cancel();
  expect(await f.controller.load()).toBe(true);
  resolve({ ...value(), observed_at: 1400 });
  expect(await pending).toBe(false);
  expect(f.controller.read().snapshot?.observed_at).toBe(1500);
  f.controller.dispose();
});
