import { createServer } from "node:http";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { parseArgs } from "node:util";
import { chromium, devices, expect } from "@playwright/test";
import { observeNativeWorkerTrace } from "../../tests/e2e/helpers/native-worker-trace.ts";

const { values } = parseArgs({
  options: { attempts: { type: "string", default: "1" } },
});
if (!/^[1-8]$/.test(values.attempts))
  throw Error("--attempts must be an integer from 1 to 8");
const attempts = Number(values.attempts);
const outputBase = resolve(".tmp");
await mkdir(outputBase, { recursive: true });
const folder = await mkdtemp(join(outputBase, "sw-restart-"));
const environment = Object.fromEntries(
  [
    "SystemRoot",
    "WINDIR",
    "PATH",
    "TEMP",
    "TMP",
    "HOME",
    "LANG",
    "DISPLAY",
    "XDG_RUNTIME_DIR",
  ]
    .filter((key) => process.env[key] !== undefined)
    .map((key) => [key, process.env[key]]),
);
const browser = await chromium.launch({
  channel: "chromium",
  headless: true,
  env: environment,
});
const results = [];

async function fixture() {
  let revision = 0;
  const server = createServer((request, response) => {
    if (request.url === "/service-worker.js") {
      response.writeHead(200, {
        "content-type": "text/javascript",
        "cache-control": "no-store",
      });
      response.end(`// revision ${revision}
self.addEventListener("activate", event => event.waitUntil(self.clients.claim()));
self.addEventListener("message", event => {
  if (event.data === "update")
    event.waitUntil(self.skipWaiting().then(() => event.ports[0]?.postMessage(true)));
});`);
    } else if (request.url === "/favicon.ico") response.writeHead(204).end();
    else {
      response.writeHead(200, {
        "content-type": "text/html",
        "cache-control": "no-store",
      });
      response.end(`<!doctype html><title>Native worker restart probe</title><h1>Native worker restart probe</h1><script>
sessionStorage.setItem("boots", String(Number(sessionStorage.getItem("boots") || 0) + 1));
let controlled = Boolean(navigator.serviceWorker.controller);
navigator.serviceWorker.addEventListener("controllerchange", () => {
  if (controlled) location.reload();
  controlled = true;
});
navigator.serviceWorker.register("/service-worker.js", { updateViaCache: "none" });
</script>`);
    }
  });
  await new Promise((accept, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", accept);
  });
  const origin = `http://127.0.0.1:${server.address().port}`;
  return {
    origin,
    advance() {
      revision++;
    },
    close: () =>
      new Promise((accept, reject) => {
        server.close((error) => (error ? reject(error) : accept()));
        server.closeIdleConnections();
      }),
  };
}

async function inspect(page) {
  return page.evaluate(async () => {
    const registration = await navigator.serviceWorker.getRegistration("/");
    return {
      boots: sessionStorage.getItem("boots"),
      active: registration?.active?.state ?? null,
      waiting: registration?.waiting?.state ?? null,
    };
  });
}

async function collect(mobile, forceRestart, attempt) {
  const host = await fixture();
  let context;
  const record = {
    mobile,
    forceRestart,
    attempt,
    restartRequested: false,
    restartedOldVersion: false,
    reloadedWithinBudget: false,
    stage: "initial",
  };
  const versions = [];
  let oldVersion;
  let request;
  let finishTrace;
  let native;
  try {
    context = await browser.newContext(
      devices[mobile ? "Pixel 7" : "Desktop Chrome"],
    );
    native = await browser.newBrowserCDPSession();
    const page = await context.newPage();
    const control = await context.newCDPSession(page);
    control.on(
      "ServiceWorker.workerVersionUpdated",
      ({ versions: updates }) => {
        for (const version of updates) {
          if (version.scriptURL !== `${host.origin}/service-worker.js`)
            continue;
          versions.push({
            at: performance.now(),
            id: version.versionId,
            status: version.status,
            running: version.runningStatus,
          });
          if (!oldVersion && version.status === "activated")
            oldVersion = version.versionId;
          if (
            forceRestart &&
            !record.restartRequested &&
            version.versionId === oldVersion &&
            version.status === "activated" &&
            version.runningStatus === "stopping"
          ) {
            record.restartRequested = true;
            request = control
              .send("ServiceWorker.startWorker", {
                scopeURL: `${host.origin}/`,
              })
              .catch(() => {});
          }
        }
      },
    );
    await control.send("ServiceWorker.enable");
    finishTrace = await observeNativeWorkerTrace(native);
    await page.goto(host.origin);
    await expect
      .poll(() =>
        page.evaluate(() => Boolean(navigator.serviceWorker.controller)),
      )
      .toBe(true);
    const other = await context.newPage();
    await other.goto(host.origin);
    for (const tab of [page, other])
      await expect
        .poll(() =>
          tab.evaluate(() => Boolean(navigator.serviceWorker.controller)),
        )
        .toBe(true);
    record.stage = "update";
    host.advance();
    await page.evaluate(async () => {
      await (await navigator.serviceWorker.getRegistration("/")).update();
    });
    await expect
      .poll(() =>
        page.evaluate(
          async () =>
            (await navigator.serviceWorker.getRegistration("/"))?.waiting
              ?.state,
        ),
      )
      .toBe("installed");
    record.stage = "load";
    // Preserve the original product scenario's 10-second observation budget.
    await Promise.all([
      page.waitForEvent("load", { timeout: 10000 }),
      other.waitForEvent("load", { timeout: 10000 }),
      page.evaluate(async () => {
        const registration = await navigator.serviceWorker.getRegistration("/");
        const channel = new MessageChannel();
        registration.waiting.postMessage("update", [channel.port2]);
      }),
    ]);
    record.reloadedWithinBudget = true;
  } catch (error) {
    record.detail = String(error.message).split("\n")[0];
    record.error =
      record.stage === "load" && error.name === "TimeoutError"
        ? "load_timeout"
        : "fixture_error";
  } finally {
    const trace = finishTrace ? await finishTrace() : null;
    try {
      const observed = versions.filter((version) => version.id === oldVersion);
      const stop = observed.findIndex(
        (version) => version.running === "stopping",
      );
      record.restartedOldVersion =
        stop >= 0 &&
        observed
          .slice(stop + 1)
          .some(
            (version) =>
              version.running === "starting" || version.running === "running",
          );
      record.states = await Promise.all((context?.pages() ?? []).map(inspect));
      record.versions = versions;
      await writeFile(
        join(folder, `${mobile}-${forceRestart}-${attempt}.json`),
        JSON.stringify({ record, trace }, null, 2),
      );
    } finally {
      try {
        await context?.close();
      } finally {
        await host.close();
      }
      if (!finishTrace) await native?.detach();
    }
  }
  // A requested CDP restart and an observed restart are distinct observations.
  void request;
  return record;
}

try {
  const versionSession = await browser.newBrowserCDPSession();
  await writeFile(
    join(folder, "version.json"),
    JSON.stringify(await versionSession.send("Browser.getVersion"), null, 2),
  );
  await versionSession.detach();
  for (const mobile of [false, true])
    for (let attempt = 0; attempt < attempts; attempt++)
      for (const forceRestart of [false, true]) {
        const result = await collect(mobile, forceRestart, attempt);
        results.push(result);
        await writeFile(
          join(folder, "results.json"),
          JSON.stringify(results, null, 2),
        );
        console.log(
          JSON.stringify({
            mobile,
            forceRestart,
            attempt,
            restartRequested: result.restartRequested,
            restartedOldVersion: result.restartedOldVersion,
            reloadedWithinBudget: result.reloadedWithinBudget,
            error: result.error,
          }),
        );
      }
} finally {
  await browser.close();
}
const control = results.filter((result) => !result.forceRestart);
const induced = results.filter((result) => result.forceRestart);
console.log(
  JSON.stringify({
    folder,
    control: {
      cases: control.length,
      reloaded: control.filter((result) => result.reloadedWithinBudget).length,
    },
    restart: {
      cases: induced.length,
      requested: induced.filter((result) => result.restartRequested).length,
      observed: induced.filter((result) => result.restartedOldVersion).length,
      reloaded: induced.filter((result) => result.reloadedWithinBudget).length,
    },
  }),
);
// Exit success means collection completed; the printed outcomes remain authoritative.
process.exitCode =
  control.some((result) => !result.reloadedWithinBudget) ||
  results.some((result) => result.error === "fixture_error")
    ? 1
    : 0;
