import { afterEach, expect, test, vi } from "vitest";
import { createAdSenseScript } from "./adsense";
afterEach(() => {
  document
    .querySelectorAll("script[data-liar-auto-ads]")
    .forEach((node) => node.remove());
});
test("configured Auto ads append the standard async Google script exactly once", async () => {
  const script = createAdSenseScript(document, "ca-pub-1234567890123456");
  expect(script.configured()).toBe(true);
  const load = script.load(new AbortController().signal);
  const node = document.querySelector<HTMLScriptElement>(
    "script[data-liar-auto-ads]",
  )!;
  expect(node.src).toBe(
    "https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js?client=ca-pub-1234567890123456",
  );
  expect(node.async).toBe(true);
  expect(node.crossOrigin).toBe("anonymous");
  node.dispatchEvent(new Event("load"));
  await load;
  await expect(script.load(new AbortController().signal)).rejects.toThrow();
  expect(document.querySelectorAll("script[data-liar-auto-ads]")).toHaveLength(
    1,
  );
});

test("missing or malformed publisher IDs never insert a network script", async () => {
  for (const id of [
    undefined,
    null,
    3,
    "",
    "ca-pub-1",
    " ca-pub-1234567890123456",
    "ca-pub-1234567890123456\n",
    "https://example.com/code.js",
  ]) {
    const port = createAdSenseScript(document, id);
    expect(port.configured()).toBe(false);
    await expect(port.load(new AbortController().signal)).rejects.toThrow(
      "unavailable",
    );
    expect(document.querySelector("script[data-liar-auto-ads]")).toBeNull();
  }
});

test("cancellation and load failure remove owned nodes and never retry", async () => {
  for (const mode of ["cancelled", "error", "aborted"] as const) {
    const signal = new AbortController();
    const port = createAdSenseScript(document, "ca-pub-1234567890123456");
    if (mode === "aborted") signal.abort();
    const pending = port.load(signal.signal);
    const rejected = expect(pending).rejects.toThrow("unavailable");
    const node = document.querySelector("script[data-liar-auto-ads]");
    if (mode === "cancelled") signal.abort();
    if (mode === "error") node!.dispatchEvent(new Event("error"));
    await rejected;
    expect(document.querySelector("script[data-liar-auto-ads]")).toBeNull();
    if (mode !== "aborted")
      await expect(port.load(new AbortController().signal)).rejects.toThrow();
  }
});

test("a pending network script has a bounded timeout and cleaned event listeners", async () => {
  vi.useFakeTimers();
  try {
    const port = createAdSenseScript(document, "ca-pub-1234567890123456");
    const pending = port.load(new AbortController().signal);
    const rejected = expect(pending).rejects.toThrow();
    const node = document.querySelector("script[data-liar-auto-ads]")!;
    await vi.advanceTimersByTimeAsync(3000);
    await rejected;
    node.dispatchEvent(new Event("load"));
    expect(document.querySelector("script[data-liar-auto-ads]")).toBeNull();
    expect(vi.getTimerCount()).toBe(0);
  } finally {
    vi.useRealTimers();
  }
});

test("DOM insertion errors and pre-existing SDK tags cannot create a second script", async () => {
  const existing = document.createElement("script");
  existing.dataset.liarAutoAds = "adsense";
  document.head.append(existing);
  const duplicate = createAdSenseScript(document, "ca-pub-1234567890123456");
  await expect(duplicate.load(new AbortController().signal)).rejects.toThrow();
  expect(document.querySelectorAll("script[data-liar-auto-ads]")).toHaveLength(
    1,
  );
  existing.remove();
  const append = vi.spyOn(document.head, "append").mockImplementation(() => {
    throw Error("blocked");
  });
  try {
    await expect(
      createAdSenseScript(document, "ca-pub-1234567890123456").load(
        new AbortController().signal,
      ),
    ).rejects.toThrow();
  } finally {
    append.mockRestore();
  }
});
