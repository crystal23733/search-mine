// @vitest-environment node
import { expect, test, vi } from "vitest";
import { createInstall } from "./install";
function environment(standalone = false, apple = false) {
  const media = Object.assign(new EventTarget(), { matches: standalone });
  const win = Object.assign(new EventTarget(), {
    navigator: { standalone: apple },
    matchMedia: vi.fn(() => media),
  });
  const port = createInstall(win as unknown as Window);
  return { win, media, port };
}
function invitation() {
  let resolve!: (choice: { outcome: string }) => void;
  const userChoice = new Promise<{ outcome: string }>((done) => {
    resolve = done;
  });
  const prompt = vi.fn(async () => {});
  const event = Object.assign(
    new Event("beforeinstallprompt", { cancelable: true }),
    { prompt, userChoice },
  );
  return { event, prompt, resolve };
}
test("a saved invitation is prompted synchronously by one user action, never automatically or twice", async () => {
  const { win, port } = environment();
  expect(port.read().status).toBe("guide");
  const first = invitation();
  win.dispatchEvent(first.event);
  expect(first.event.defaultPrevented).toBe(true);
  expect(port.read().status).toBe("available");
  expect(first.prompt).not.toHaveBeenCalled();
  const pending = port.install();
  expect(first.prompt).toHaveBeenCalledTimes(1);
  expect(port.read().status).toBe("prompting");
  expect(await port.install()).toBe(false);
  first.resolve({ outcome: "dismissed" });
  expect(await pending).toBe(false);
  expect(port.read()).toEqual({ status: "guide", feedback: "dismissed" });
  win.dispatchEvent(first.event);
  expect(await port.install()).toBe(false);
  expect(first.prompt).toHaveBeenCalledTimes(1);
  port.dispose();
});
test("acceptance requests installation; browser completion and current app window are distinct", async () => {
  const { win, media, port } = environment();
  const first = invitation();
  win.dispatchEvent(first.event);
  const pending = port.install();
  first.resolve({ outcome: "accepted" });
  expect(await pending).toBe(true);
  expect(port.read().status).toBe("requested");
  win.dispatchEvent(new Event("appinstalled"));
  expect(port.read().status).toBe("installed");
  media.matches = true;
  media.dispatchEvent(new Event("change"));
  expect(port.read().status).toBe("standalone");
  media.matches = false;
  media.dispatchEvent(new Event("change"));
  expect(port.read().status).toBe("installed");
  const later = invitation();
  win.dispatchEvent(later.event);
  expect(await port.install()).toBe(false);
  expect(later.prompt).not.toHaveBeenCalled();
  port.dispose();
});
test("initial standalone and Apple app windows never offer an installation invitation", async () => {
  for (const [mode, apple] of [
    [true, false],
    [false, true],
  ]) {
    const { port, win } = environment(mode, apple);
    const event = invitation();
    win.dispatchEvent(event.event);
    expect(port.read().status).toBe("standalone");
    expect(await port.install()).toBe(false);
    port.dispose();
  }
  const port = createInstall();
  expect(await port.install()).toBe(false);
  port.dispose();
});
test("dismissal or a thrown/rejected prompt can recover with a fresh browser invitation", async () => {
  for (const failure of [
    () => {
      throw Error("private detail");
    },
    () => Promise.reject(Error("private detail")),
  ]) {
    const { win, port } = environment();
    const first = invitation();
    first.event.prompt = vi.fn(failure);
    win.dispatchEvent(first.event);
    expect(await port.install()).toBe(false);
    expect(port.read()).toEqual({ status: "guide", feedback: "failed" });
    const fresh = invitation();
    win.dispatchEvent(fresh.event);
    expect(port.read()).toEqual({ status: "available", feedback: "none" });
    const pending = port.install();
    fresh.resolve({ outcome: "accepted" });
    expect(await pending).toBe(true);
    port.dispose();
  }
});
test("browser completion or display changes supersede late prompt results", async () => {
  for (const change of ["appinstalled", "display"]) {
    const { win, media, port } = environment();
    const first = invitation();
    win.dispatchEvent(first.event);
    const pending = port.install();
    if (change === "display") {
      media.matches = true;
      media.dispatchEvent(new Event("change"));
    } else win.dispatchEvent(new Event("appinstalled"));
    const snapshot = port.read();
    first.resolve({ outcome: "dismissed" });
    await pending;
    expect(port.read()).toEqual(snapshot);
    port.dispose();
  }
});
test("unsubscribe and disposal release listeners and ignore pending completion or future browser events", async () => {
  const { win, media, port } = environment();
  const listener = vi.fn();
  const remove = port.subscribe(listener);
  const first = invitation();
  win.dispatchEvent(first.event);
  expect(listener).toHaveBeenCalled();
  remove();
  listener.mockClear();
  const pending = port.install();
  port.dispose();
  port.dispose();
  const snapshot = port.read();
  first.resolve({ outcome: "accepted" });
  await pending;
  media.matches = true;
  media.dispatchEvent(new Event("change"));
  win.dispatchEvent(new Event("appinstalled"));
  const later = invitation();
  win.dispatchEvent(later.event);
  expect(later.event.defaultPrevented).toBe(false);
  expect(port.read()).toEqual(snapshot);
  expect(listener).not.toHaveBeenCalled();
  expect(await port.install()).toBe(false);
});
test("malformed invitations and choices fail without inventing an installed state", async () => {
  const { win, port } = environment();
  const malformed = new Event("beforeinstallprompt", { cancelable: true });
  win.dispatchEvent(malformed);
  expect(malformed.defaultPrevented).toBe(false);
  expect(port.read().status).toBe("guide");
  const first = invitation();
  win.dispatchEvent(first.event);
  const pending = port.install();
  first.resolve({ outcome: "invented" });
  expect(await pending).toBe(false);
  expect(port.read()).toEqual({ status: "guide", feedback: "failed" });
  port.dispose();
});
