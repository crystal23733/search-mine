import { afterEach, describe, expect, it, vi } from "vitest";
import { createGoogleConsentAdapter } from "./consent-google";
import { syntheticGoogleConsent } from "../../../../tests/fixtures/google-consent.mjs";
import type { ConsentContextPort } from "./consent-guard";
afterEach(() => vi.useRealTimers());
function fixture() {
  let owner: object | null = {};
  const changes = new Set<() => void>();
  const context: ConsentContextPort = {
    capture: () => owner,
    subscribe: (fn) => {
      changes.add(fn);
      return () => changes.delete(fn);
    },
    invalidate: () => {
      owner = {};
      for (const fn of changes) fn();
    },
  };
  let callback: ((data: unknown, success: boolean) => void) | undefined;
  const api = vi.fn(
    (
      command: string,
      _version: number,
      fn: (data: unknown, success: boolean) => void,
    ) => {
      if (command === "addEventListener") callback = fn;
    },
  );
  const browser = {
    location: { pathname: "/ko/consent", hash: "" },
    __tcfapi: api,
    googlefc: {
      callbackQueue: [] as Record<string, () => void>[],
      showRevocationMessage: vi.fn(),
    },
  };
  const client = { state: () => "ready" as const, subscribe: () => () => {} };
  const adapter = createGoogleConsentAdapter(
    browser as never,
    context,
    client,
    () => true,
    vi.fn(),
  );
  const ready = () =>
    browser.googlefc.callbackQueue.forEach((item) =>
      item.CONSENT_API_READY?.(),
    );
  const publish = (
    choice = syntheticGoogleConsent(Math.floor(Date.now() / 1000)),
  ) => callback?.({ ...choice, listenerId: 12 }, true);
  return {
    adapter,
    ready,
    publish,
    api,
    browser,
    context,
    hide() {
      owner = null;
      for (const fn of changes) fn();
    },
  };
}
describe("current Google choice", () => {
  it("waits for the supported API and separates own purposes from Google ads", () => {
    const f = fixture();
    expect(f.adapter.read()).toEqual({ status: "unknown" });
    f.ready();
    f.publish();
    expect(f.adapter.read()).toMatchObject({
      status: "valid",
      analytics: true,
      advertising: true,
    });
    expect(f.adapter.evidence()?.provider).toBe("google_tcf_v2");
    f.publish(
      syntheticGoogleConsent(Math.floor(Date.now() / 1000), {
        advertising: true,
        analytics: false,
        measurement: false,
      }),
    );
    expect(f.adapter.read()).toMatchObject({
      analytics: false,
      advertising: true,
    });
    f.adapter.dispose();
    expect(f.adapter.evidence()).toBeUndefined();
  });
  it("invalidates expired, hidden, unsuccessful, out of region and late callbacks", () => {
    vi.useFakeTimers();
    const f = fixture();
    f.ready();
    f.publish();
    vi.advanceTimersByTime(60001);
    expect(f.adapter.evidence()).toBeUndefined();
    f.publish();
    expect(f.adapter.evidence()).toBeDefined();
    f.hide();
    f.publish();
    expect(f.adapter.read()).toEqual({ status: "unknown" });
    f.adapter.dispose();
    const g = fixture();
    g.ready();
    g.publish({ ...syntheticGoogleConsent(), gdprApplies: false });
    expect(g.adapter.evidence()).toBeUndefined();
    g.adapter.dispose();
  });
  it("broadcasts only user changes and uses the official revocation method", async () => {
    const f = fixture();
    const changed = vi.fn();
    f.adapter.subscribeBroadcast(changed);
    f.ready();
    f.publish();
    expect(changed).not.toHaveBeenCalled();
    f.publish({
      ...syntheticGoogleConsent(),
      eventStatus: "useractioncomplete",
    });
    expect(changed).toHaveBeenCalledTimes(1);
    await f.adapter.reopen(new AbortController().signal);
    expect(f.browser.googlefc.showRevocationMessage).toHaveBeenCalledTimes(1);
    f.adapter.dispose();
  });
});
