import type { ActivityPort } from "./activity";
export interface OfflineState {
  online: boolean;
  ready: boolean;
  waiting: boolean;
  working: boolean;
  disabled: boolean;
  error: boolean;
}
export interface OfflinePort {
  read(): OfflineState;
  subscribe(listener: () => void): () => void;
  update(): Promise<boolean>;
  clear(): Promise<boolean>;
  enable(): Promise<boolean>;
}
export function createOffline(
  activity: ActivityPort,
  options?: {
    window: Window;
    enabled: boolean;
    storage?: Pick<Storage, "getItem" | "setItem">;
  },
): OfflinePort {
  const win = options?.window;
  let disabled = false;
  try {
    disabled = options?.storage?.getItem("liar.cache.disabled.v1") === "true";
  } catch {
    /* Memory preference still works. */
  }
  let state: OfflineState = {
    online: win?.navigator.onLine ?? true,
    ready: false,
    waiting: false,
    working: false,
    disabled,
    error: false,
  };
  const listeners = new Set<() => void>();
  const publish = (patch: Partial<OfflineState>) => {
    state = { ...state, ...patch };
    listeners.forEach((listener) => listener());
  };
  let container: ServiceWorkerContainer | undefined;
  try {
    container = win?.navigator.serviceWorker;
  } catch {
    state.error = true;
  }
  const supported = Boolean(options?.enabled && container);
  let registration: ServiceWorkerRegistration | undefined;
  let registering: Promise<boolean> | undefined;
  let controlled = Boolean(container?.controller);
  const setDisabled = (value: boolean) => {
    try {
      options?.storage?.setItem("liar.cache.disabled.v1", String(value));
    } catch {
      /* Memory preference still works. */
    }
    publish({ disabled: value });
  };
  const inspect = () =>
    publish({
      ready: !state.disabled && registration?.active?.state === "activated",
      waiting: Boolean(registration?.waiting),
    });
  const register = () => {
    if (registering) return registering;
    registering = (async () => {
      if (!supported || state.disabled) return false;
      try {
        registration = state.online
          ? await container!.register("/service-worker.js", {
              scope: "/",
              updateViaCache: "none",
            })
          : await container!.getRegistration("/");
        if (!registration) return false;
        registration.addEventListener("updatefound", () => {
          registration?.installing?.addEventListener("statechange", inspect);
          inspect();
        });
        registration.installing?.addEventListener("statechange", inspect);
        registration.active?.addEventListener("statechange", inspect);
        inspect();
        return true;
      } catch {
        publish({ error: true });
        return false;
      }
    })().finally(() => {
      registering = undefined;
    });
    return registering;
  };
  win?.addEventListener("online", () => {
    publish({ online: true });
    if (!registration) void register();
  });
  win?.addEventListener("offline", () => publish({ online: false }));
  if (supported) {
    container!.addEventListener("message", (event) => {
      const source = event.source;
      if (!source || !("scriptURL" in source)) return;
      const url = new URL(source.scriptURL);
      if (
        url.origin !== win!.location.origin ||
        url.pathname !== "/service-worker.js"
      )
        return;
      if (
        event.data?.type === "prepare" &&
        typeof event.data.token === "string"
      )
        event.ports[0]?.postMessage(activity.prepare(event.data.token));
      else if (
        event.data?.type === "release" &&
        typeof event.data.token === "string"
      )
        activity.release(event.data.token);
      else if (event.data?.type === "cache-disabled") {
        setDisabled(true);
        publish({ ready: false, waiting: false });
      }
    });
    container!.addEventListener("controllerchange", () => {
      const reload = controlled && activity.read().locked;
      controlled = Boolean(container!.controller);
      inspect();
      if (reload) win!.location.reload();
    });
    if (!disabled) void register();
  }
  const request = async (type: "update" | "clear-cache") => {
    if (
      !supported ||
      state.working ||
      activity.read().busy ||
      activity.read().locked
    )
      return false;
    const worker =
      type === "update" ? registration?.waiting : registration?.active;
    if (!worker) return false;
    publish({ working: true, error: false });
    const result = await new Promise<boolean>((resolve) => {
      const channel = new MessageChannel();
      let settled = false;
      const done = (value: boolean) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        channel.port1.close();
        channel.port2.close();
        resolve(value);
      };
      const timer = setTimeout(() => done(false), 8000);
      channel.port1.onmessage = (event) => done(event.data === true);
      channel.port1.onmessageerror = () => done(false);
      try {
        worker.postMessage({ type }, [channel.port2]);
      } catch {
        done(false);
      }
    });
    publish({ working: false, error: !result });
    if (result && type === "clear-cache") {
      registration = undefined;
      setDisabled(true);
      publish({ ready: false, waiting: false });
    }
    return result;
  };
  return {
    read: () => ({ ...state }),
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    update: () => request("update"),
    clear: () => request("clear-cache"),
    enable: async () => {
      if (activity.read().locked || state.working) return false;
      setDisabled(false);
      publish({ error: false });
      return register();
    },
  };
}
