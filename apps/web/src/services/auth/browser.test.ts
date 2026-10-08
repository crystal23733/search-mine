import { expect, test, vi } from "vitest";
import { createBrowserAuth } from "./browser";
import { PROVIDERS, type AuthTransport } from "./types";
test("offline, visible-page and peer invalidation reconcile server authority without publishing personal data", async () => {
  const account = {
    id: "a0000000-0000-4000-8000-000000000001",
    nickname: "player",
  };
  const bootstrap = {
    account,
    session_revision: "b0000000-0000-4000-8000-000000000001",
    csrf: "fixture-csrf",
    providers: PROVIDERS.map((p) => ({ provider: p.id, available: true })),
  };
  const transport: AuthTransport = {
    bootstrap: vi.fn().mockResolvedValue(bootstrap),
    identities: vi
      .fn()
      .mockResolvedValue([{ provider: "google", linked_at: 1 }]),
    nickname: vi.fn().mockResolvedValue({ ...account, nickname: "changed" }),
    start: vi.fn(),
    export: vi.fn(),
    erase: vi.fn(),
    unlink: vi.fn(),
    logout: vi.fn(),
  };
  const channel = Object.assign(new EventTarget(), {
    postMessage: vi.fn(),
    close: vi.fn(),
  });
  const browser = createBrowserAuth(window, transport, channel);
  try {
    await browser.auth.refresh();
    window.dispatchEvent(new Event("offline"));
    expect(browser.auth.account()).toBeNull();
    window.dispatchEvent(new Event("online"));
    await vi.waitFor(() => expect(browser.auth.connected()).toBe(true));
    await browser.auth.nickname("changed", account.id);
    expect(channel.postMessage.mock.calls).toEqual([["invalidate"]]);
    vi.mocked(transport.bootstrap).mockResolvedValue({
      ...bootstrap,
      account: null,
      session_revision: null,
    });
    channel.dispatchEvent(new MessageEvent("message", { data: "invalidate" }));
    await vi.waitFor(() => expect(browser.auth.read().status).toBe("ready"));
    expect(browser.auth.account()).toBeNull();
    const before = vi.mocked(transport.bootstrap).mock.calls.length;
    channel.dispatchEvent(
      new MessageEvent("message", { data: { account, csrf: "secret" } }),
    );
    expect(transport.bootstrap).toHaveBeenCalledTimes(before);
  } finally {
    browser.dispose();
  }
  expect(channel.close).toHaveBeenCalledTimes(1);
});
