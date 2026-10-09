import type {
  BrowserContext,
  ConsoleMessage,
  Page,
  TestInfo,
} from "@playwright/test";
import { mkdir, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";

async function observeWithinBudget<T>(observation: Promise<T>): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      observation,
      new Promise<never>((_, reject) => {
        timer = setTimeout(() => reject(Error("diagnostic unavailable")), 1000);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

// Keep observations in the runner so a successful reload cannot erase its cause.
export async function observeWorkerUpdate(
  context: BrowserContext,
  page: Page,
  origin: string,
): Promise<(info: TestInfo) => Promise<void>> {
  const events: unknown[] = [];
  const prefix = "offline-update-probe:";
  const onConsole = (message: ConsoleMessage) => {
    if (message.type() !== "debug" || !message.text().startsWith(prefix))
      return;
    events.push(JSON.parse(message.text().slice(prefix.length)));
  };
  context.on("console", onConsole);
  const native = await context.newCDPSession(page);
  native.on("ServiceWorker.workerVersionUpdated", ({ versions }) => {
    for (const version of versions)
      if (version.scriptURL === `${origin}/service-worker.js`)
        events.push({ type: "native-worker", at: Date.now(), ...version });
  });
  await native.send("ServiceWorker.enable");
  await context.addInitScript(
    ({ prefix, origin }) => {
      if (location.origin !== origin) return;
      const record = (type: string, data: object = {}) =>
        console.debug(
          prefix +
            JSON.stringify({
              type,
              at: performance.timeOrigin + performance.now(),
              page: location.pathname,
              boot: sessionStorage.getItem("offline-test-boots"),
              ...data,
            }),
        );
      sessionStorage.setItem(
        "offline-test-boots",
        String(Number(sessionStorage.getItem("offline-test-boots") ?? 0) + 1),
      );
      record("boot");
      addEventListener("load", () => record("load"));
      addEventListener("beforeunload", () => record("beforeunload"));
      document.addEventListener("click", (event) => {
        const button = (event.target as Element | null)?.closest("button");
        if (button?.textContent?.includes("Apply update")) record("click");
      });
      const send = ServiceWorker.prototype.postMessage;
      ServiceWorker.prototype.postMessage = function (...args) {
        const type = args[0]?.type;
        if (type === "update" || type === "clear-cache")
          record("request", { command: type, worker: this.state });
        return Reflect.apply(send, this, args);
      };
      navigator.serviceWorker.addEventListener("controllerchange", () =>
        record("controllerchange", {
          worker: navigator.serviceWorker.controller?.state ?? null,
        }),
      );
      navigator.serviceWorker.addEventListener("message", (event) => {
        if (!["prepare", "release"].includes(event.data?.type)) return;
        record(event.data.type, { token: event.data.token });
        const port = event.ports[0];
        if (event.data.type !== "prepare" || !port) return;
        const reply = port.postMessage.bind(port);
        port.postMessage = (value: unknown) => {
          record("reply", { token: event.data.token, value });
          reply(value);
        };
      });
    },
    { prefix, origin },
  );
  return async (info) => {
    for (const tab of context.pages()) {
      try {
        events.push(
          await observeWithinBudget(
            tab.evaluate(async () => {
              const registration =
                await navigator.serviceWorker.getRegistration("/");
              return {
                type: "final-state",
                at: performance.timeOrigin + performance.now(),
                page: location.pathname,
                boot: sessionStorage.getItem("offline-test-boots"),
                controller: navigator.serviceWorker.controller?.state ?? null,
                active: registration?.active?.state ?? null,
                waiting: registration?.waiting?.state ?? null,
                installing: registration?.installing?.state ?? null,
                apply: Array.from(document.querySelectorAll("button"))
                  .filter((button) =>
                    button.textContent?.includes("Apply update"),
                  )
                  .map((button) => ({ disabled: button.disabled })),
              };
            }),
          ),
        );
      } catch (error) {
        events.push({ type: "unavailable", message: String(error) });
      }
    }
    context.off("console", onConsole);
    try {
      await observeWithinBudget(native.detach());
    } catch (error) {
      events.push({ type: "observer-detach", message: String(error) });
    }
    const body = JSON.stringify(events, null, 2);
    const id = createHash("sha256")
      .update(info.testId)
      .digest("hex")
      .slice(0, 16);
    await mkdir(".tmp/offline-update-probes", { recursive: true });
    await writeFile(
      `.tmp/offline-update-probes/${id}-${info.repeatEachIndex}-${info.retry}-${Date.now()}.json`,
      body,
    );
    await info.attach("offline-update-events.json", {
      body,
      contentType: "application/json",
    });
  };
}
