import { expect, test, vi } from "vitest";
import { createActivity } from "./activity";
import { createOffline } from "./offline";
function environment() {
  const worker = Object.assign(new EventTarget(), {
    scriptURL: "https://liar.test/service-worker.js",
    state: "activated",
    postMessage: vi.fn((_data: unknown, ports: MessagePort[]) =>
      ports[0].postMessage(true),
    ),
  });
  const registration = Object.assign(new EventTarget(), {
    active: worker,
    waiting: null as typeof worker | null,
    installing: null as typeof worker | null,
  });
  const container = Object.assign(new EventTarget(), {
    controller: null as typeof worker | null,
    register: vi.fn(async () => registration),
    getRegistration: vi.fn(async () => registration),
  });
  const win = Object.assign(new EventTarget(), {
    navigator: { onLine: true, serviceWorker: container },
    location: { origin: "https://liar.test", reload: vi.fn() },
  });
  const message = (data: unknown, source = worker, reply = vi.fn()) => {
    const event = Object.assign(new Event("message"), {
      data,
      source,
      ports: [{ postMessage: reply }],
    });
    container.dispatchEvent(event);
    return reply;
  };
  return {
    win,
    worker,
    registration,
    container,
    message,
    options: { window: win as unknown as Window, enabled: true },
  };
}
test("offline reload reads an existing active registration and network recovery remains a signal, not authentication", async () => {
  const env = environment();
  env.win.navigator.onLine = false;
  const port = createOffline(createActivity(), env.options);
  const changed = vi.fn(),
    unsubscribe = port.subscribe(changed);
  await vi.waitFor(() => expect(port.read().ready).toBe(true));
  expect(env.container.register).not.toHaveBeenCalled();
  expect(env.container.getRegistration).toHaveBeenCalledWith("/");
  env.win.dispatchEvent(new Event("online"));
  expect(port.read().online).toBe(true);
  env.win.dispatchEvent(new Event("offline"));
  expect(port.read().online).toBe(false);
  expect(changed).toHaveBeenCalled();
  unsubscribe();
});
test("only a trusted worker can prepare locks; first claim does not reload and locked old-controller change does", async () => {
  const env = environment(),
    activity = createActivity();
  createOffline(activity, env.options);
  await vi.waitFor(() =>
    expect(env.container.register).toHaveBeenCalledTimes(1),
  );
  env.container.controller = env.worker;
  env.container.dispatchEvent(new Event("controllerchange"));
  expect(env.win.location.reload).not.toHaveBeenCalled();
  env.message(
    { type: "prepare", token: "foreign" },
    { ...env.worker, scriptURL: "https://other.test/service-worker.js" },
  );
  expect(activity.read().locked).toBe(false);
  const reply = env.message({ type: "prepare", token: "idle" });
  expect(reply).toHaveBeenCalledWith(true);
  expect(() => activity.hold()).toThrow("updating");
  env.message({ type: "release", token: "idle" });
  const release = activity.hold();
  expect(env.message({ type: "prepare", token: "busy" })).toHaveBeenCalledWith(
    false,
  );
  release();
  env.message({ type: "prepare", token: "update" });
  env.container.dispatchEvent(new Event("controllerchange"));
  expect(env.win.location.reload).toHaveBeenCalledTimes(1);
  env.message({ type: "release", token: "update" });
});
test("cache disabled preference survives clearing, can re-enable, and missing worker or busy game cannot request activation", async () => {
  const env = environment(),
    activity = createActivity();
  const storage = { getItem: vi.fn(() => "true"), setItem: vi.fn() };
  const port = createOffline(activity, { ...env.options, storage });
  expect(port.read().disabled).toBe(true);
  expect(await port.update()).toBe(false);
  expect(await port.enable()).toBe(true);
  expect(port.read().ready).toBe(true);
  const release = activity.hold();
  expect(await port.clear()).toBe(false);
  expect(env.worker.postMessage).not.toHaveBeenCalled();
  release();
  env.registration.waiting = env.worker;
  env.registration.dispatchEvent(new Event("updatefound"));
  expect(port.read().waiting).toBe(true);
  expect(await port.update()).toBe(true);
  expect(await port.clear()).toBe(true);
  expect(port.read().disabled).toBe(true);
  expect(storage.setItem).toHaveBeenLastCalledWith(
    "liar.cache.disabled.v1",
    "true",
  );
  env.message({ type: "cache-disabled" });
  expect(port.read().ready).toBe(false);
});
test("denied registration/storage and silent worker have bounded failures without touching local games", async () => {
  const env = environment(),
    activity = createActivity();
  env.container.register.mockRejectedValueOnce(Error("denied"));
  const storage = {
    getItem: () => {
      throw Error("denied");
    },
    setItem: () => {
      throw Error("denied");
    },
  };
  const port = createOffline(activity, { ...env.options, storage });
  await vi.waitFor(() => expect(port.read().error).toBe(true));
  env.win.dispatchEvent(new Event("online"));
  await vi.waitFor(() => expect(port.read().ready).toBe(true));
  env.worker.postMessage.mockImplementation(() => {});
  vi.useFakeTimers();
  try {
    const request = port.clear();
    expect(port.read().working).toBe(true);
    await vi.advanceTimersByTimeAsync(8000);
    expect(await request).toBe(false);
    expect(port.read()).toMatchObject({
      working: false,
      error: true,
      disabled: false,
    });
  } finally {
    vi.useRealTimers();
  }
  expect(activity.read()).toEqual({ busy: false, locked: false });
  Object.defineProperty(env.win.navigator, "serviceWorker", {
    get: () => {
      throw Error("denied");
    },
  });
  expect(createOffline(activity, env.options).read().error).toBe(true);
});
