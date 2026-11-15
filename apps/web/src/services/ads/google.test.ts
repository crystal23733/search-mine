import { afterEach, expect, test, vi } from "vitest";
import { createGooglePublisherClient } from "./google";
const publisher = "ca-pub-1234567890123456";
const namespace = () =>
  (window as Window & { adsbygoogle?: unknown }).adsbygoogle as
    { pauseAdRequests: number } | undefined;
afterEach(() => {
  document
    .querySelectorAll("script[data-liar-auto-ads],script[data-fixture-nonce]")
    .forEach((node) => node.remove());
  delete (window as Window & { adsbygoogle?: unknown }).adsbygoogle;
  vi.useRealTimers();
});
test("Google messaging pauses requests before a single nonce-bearing tag is inserted", async () => {
  const entry = document.createElement("script");
  entry.nonce = "per-response-nonce";
  entry.dataset.fixtureNonce = "true";
  document.head.append(entry);
  const client = createGooglePublisherClient(document, true, publisher);
  const original = document.head.append.bind(document.head);
  const append = vi
    .spyOn(document.head, "append")
    .mockImplementation((...nodes) => {
      expect(namespace()?.pauseAdRequests).toBe(1);
      original(...nodes);
    });
  const signal = new AbortController();
  const one = client.prepare(signal.signal),
    two = client.prepare(signal.signal);
  expect(client.prepared()).toBe(true);
  const node = document.querySelector<HTMLScriptElement>(
    "script[data-liar-auto-ads]",
  )!;
  expect(node.src).toBe(
    `https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js?client=${publisher}`,
  );
  expect(node.nonce).toBe("per-response-nonce");
  expect(node.referrerPolicy).toBe("strict-origin");
  node.dispatchEvent(new Event("load"));
  await Promise.all([one, two]);
  expect(client.state()).toBe("ready");
  expect(namespace()?.pauseAdRequests).toBe(1);
  await client.load(signal.signal);
  expect(namespace()?.pauseAdRequests).toBe(0);
  signal.abort();
  expect(namespace()?.pauseAdRequests).toBe(1);
  expect(document.querySelectorAll("script[data-liar-auto-ads]")).toHaveLength(
    1,
  );
  append.mockRestore();
  client.dispose();
});
test("disabled or missing Google configuration creates no namespace, tag or attempt", async () => {
  for (const [enabled, id] of [
    [false, publisher],
    [true, undefined],
    [true, "ca-pub-1"],
  ] as const) {
    const client = createGooglePublisherClient(document, enabled, id);
    expect(client.configured()).toBe(false);
    await expect(client.prepare(new AbortController().signal)).rejects.toThrow(
      "unavailable",
    );
    expect(namespace()).toBeUndefined();
    expect(client.prepared()).toBe(false);
    client.dispose();
  }
});
test("failed or cancelled bootstrap stays paused and never retries in the same document", async () => {
  const client = createGooglePublisherClient(document, true, publisher),
    signal = new AbortController();
  const pending = client.prepare(signal.signal),
    rejected = expect(pending).rejects.toThrow("unavailable");
  signal.abort();
  await rejected;
  expect(client.state()).toBe("error");
  expect(client.prepared()).toBe(true);
  expect(namespace()?.pauseAdRequests).toBe(1);
  await expect(client.prepare(new AbortController().signal)).rejects.toThrow();
  expect(document.querySelector("script[data-liar-auto-ads]")).toBeNull();
  client.dispose();
});
test("pause controls follow the SDK's current namespace after bootstrap", async () => {
  const client = createGooglePublisherClient(document, true, publisher);
  const signal = new AbortController();
  const pending = client.prepare(signal.signal);
  const sdk = { pauseAdRequests: 1 };
  (window as Window & { adsbygoogle?: unknown }).adsbygoogle = sdk;
  document
    .querySelector("script[data-liar-auto-ads]")!
    .dispatchEvent(new Event("load"));
  await pending;
  await client.load(signal.signal);
  expect(sdk.pauseAdRequests).toBe(0);
  signal.abort();
  expect(sdk.pauseAdRequests).toBe(1);
  client.dispose();
});
