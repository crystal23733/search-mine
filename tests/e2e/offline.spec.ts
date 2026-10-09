import { test as base, expect } from "@playwright/test";
import {
  createWorkerUpdateServer,
  type WorkerUpdateServer,
} from "./helpers/worker-update-server";
import { readFileSync, mkdirSync } from "node:fs";
import type { DailyStep } from "../../packages/protocol/src/index";
const test = base.extend<{ workerUpdate: WorkerUpdateServer }>({
  workerUpdate: async ({ browserName }, use) => {
    expect(browserName).toBe("chromium");
    const server = await createWorkerUpdateServer();
    try {
      await use(server);
    } finally {
      await server.close();
    }
  },
});
const daily = JSON.parse(
  readFileSync(
    new URL("../fixtures/native-wasm.json", import.meta.url),
    "utf8",
  ),
).daily as { events: Array<{ input: string; expected: DailyStep }> };
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() =>
    localStorage.setItem("liar.tutorial.v1", '"skipped"'),
  );
});
test("production precaches public WASM and all locales; offline reload starts a real daily without creating an account", async ({
  page,
  context,
}, info) => {
  test.setTimeout(60_000);
  await page.goto("/en/settings");
  await expect
    .poll(
      () => page.evaluate(() => Boolean(navigator.serviceWorker.controller)),
      { timeout: 10_000 },
    )
    .toBe(true);
  const cache = await page.evaluate(async () => {
    const keys = await caches.keys();
    return (
      await Promise.all(
        keys.map(async (key) =>
          (await (await caches.open(key)).keys()).map((request) => request.url),
        ),
      )
    ).flat();
  });
  expect(cache.some((url) => url.includes("liar_wasm_bg.wasm"))).toBe(true);
  expect(
    cache.filter((url) =>
      /\/api\/|oauth|session|doubleclick|googlesyndication/.test(url),
    ),
  ).toEqual([]);
  await context.setOffline(true);
  for (const locale of ["en", "ko", "ja", "zh-CN", "es", "pt-BR", "de", "fr"]) {
    await page.goto(`/${locale}/settings`);
    await expect(page.locator("html")).toHaveAttribute("lang", locale);
    await expect(page.locator("header select")).toHaveValue(locale);
  }
  await page.goto("/en/daily");
  await expect(page.getByRole("gridcell")).toHaveCount(256);
  await expect(page.getByTestId("own-progress")).not.toHaveText("0 / 216");
  await expect(
    page.getByText(
      "Offline: cached local play is available. Online matches and official records require a connection.",
    ),
  ).toBeVisible();
  if (process.env.LIAR_CAPTURE_DESIGN === "1") {
    mkdirSync(".tmp/screenshots", { recursive: true });
    await page.evaluate(() => window.scrollTo(0, 0));
    await page.screenshot({
      path: `.tmp/screenshots/14-offline-${info.project.name}.png`,
      fullPage: true,
    });
  }
  await page.goto("/en/practice");
  await expect(page.getByRole("gridcell")).toHaveCount(256);
  await expect(page.getByTestId("bot-progress")).toBeVisible();
});
test("offline completion atomically survives reload with a pending replay; settings clear both after confirmation", async ({
  page,
  context,
}, info) => {
  test.setTimeout(60_000);
  await page.clock.setFixedTime(new Date("2026-10-07T12:00:00Z"));
  await page.goto("/en/settings");
  await expect(
    page.getByText("Public local play assets are cached on this device."),
  ).toBeVisible();
  await context.setOffline(true);
  await page.goto("/en/daily");
  await expect(page.getByRole("gridcell")).toHaveCount(256);
  await expect(page.getByRole("grid")).toHaveAttribute(
    "aria-readonly",
    "false",
  );
  for (const event of daily.events) {
    const action = JSON.parse(event.input).action;
    await page.locator(`[data-cell="${action.cell}"]`).press("Enter");
    await expect(page.locator(`[data-cell="${action.cell}"]`)).toHaveAttribute(
      "data-state",
      "safe",
    );
  }
  await expect(page.getByText("First local completion saved.")).toBeVisible();
  await page.reload();
  await expect(page.locator(".daily-records li")).toHaveCount(1);
  await page.goto("/en/settings");
  await expect(page.getByText("Pending local results: 1")).toBeVisible();
  if (process.env.LIAR_CAPTURE_DESIGN === "1") {
    mkdirSync(".tmp/screenshots", { recursive: true });
    await page.screenshot({
      path: `.tmp/screenshots/14-settings-${info.project.name}.png`,
      fullPage: true,
    });
  }
  const before = await page.evaluate(async () => {
    const request = indexedDB.open("liar.daily.v1");
    const db = await new Promise<IDBDatabase>((resolve, reject) => {
      request.onsuccess = () => resolve(request.result);
      request.onerror = reject;
    });
    const tx = db.transaction("pending");
    const get = tx.objectStore("pending").getAll();
    const rows = await new Promise<unknown[]>((resolve, reject) => {
      get.onsuccess = () => resolve(get.result);
      get.onerror = reject;
    });
    db.close();
    return rows;
  });
  expect(before).toHaveLength(1);
  expect(before[0]).toMatchObject({ status: "unverified", replay: { v: 1 } });
  expect(JSON.stringify(before)).not.toMatch(
    /account|nickname|email|password|verified_at/,
  );
  await page
    .getByRole("button", { name: "Delete local records", exact: true })
    .click();
  await expect(
    page.getByRole("dialog", { name: "Delete local records" }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Delete records", exact: true })
    .click();
  await expect(page.getByText("Pending local results: 0")).toBeVisible();
  await page.reload();
  await expect(page.getByText("Pending local results: 0")).toBeVisible();
  await page.goto("/en/daily");
  await expect(page.locator(".daily-records li")).toHaveCount(0);
});
test("two-tab update cannot interrupt a game; idle update reloads both, cache deletion is scoped and can be re-enabled", async ({
  page,
  context,
  workerUpdate,
}) => {
  test.setTimeout(60_000);
  await context.addInitScript(() =>
    sessionStorage.setItem(
      "offline-test-boots",
      String(Number(sessionStorage.getItem("offline-test-boots") ?? 0) + 1),
    ),
  );
  await page.goto(`${workerUpdate.origin}/en/settings`);
  await expect(
    page.getByText("Public local play assets are cached on this device."),
  ).toBeVisible();
  const other = await context.newPage();
  await other.goto(`${workerUpdate.origin}/en/daily`);
  await expect(other.getByRole("gridcell")).toHaveCount(256);
  const progress = await other.getByTestId("own-progress").textContent();
  for (const tab of [page, other])
    await expect
      .poll(() =>
        tab.evaluate(() => navigator.serviceWorker.controller?.scriptURL),
      )
      .toBe(`${workerUpdate.origin}/service-worker.js`);
  workerUpdate.advance();
  await page.evaluate(async () => {
    await (await navigator.serviceWorker.getRegistration("/"))!.update();
  });
  await expect(
    page.getByRole("button", { name: "Apply update" }),
  ).toBeVisible();
  await expect(
    other.getByRole("button", { name: "Apply update" }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "Apply update" }).click();
  await expect(
    page.getByText(
      "The operation could not finish. Another tab may be playing, or storage may be unavailable. Try again from home.",
    ),
  ).toBeVisible();
  await expect(other.getByTestId("own-progress")).toHaveText(progress!);
  expect(
    await other.evaluate(() => sessionStorage.getItem("offline-test-boots")),
  ).toBe("1");
  await other.getByRole("link", { name: "Home", exact: true }).click();
  await expect(other.getByRole("gridcell")).toHaveCount(0);
  // Route removal precedes the game effect's release of its activity hold.
  await expect(
    other.getByRole("button", { name: "Apply update" }),
  ).toBeEnabled();
  await expect(
    page.getByRole("button", { name: "Apply update" }),
  ).toBeEnabled();
  await Promise.all([
    page.waitForEvent("load"),
    other.waitForEvent("load"),
    page.getByRole("button", { name: "Apply update" }).click(),
  ]);
  await expect
    .poll(() =>
      page.evaluate(() => sessionStorage.getItem("offline-test-boots")),
    )
    .toBe("2");
  await expect
    .poll(() =>
      other.evaluate(() => sessionStorage.getItem("offline-test-boots")),
    )
    .toBe("2");
  await expect(
    page.getByText("Public local play assets are cached on this device."),
  ).toBeVisible();
  await page.evaluate(async () => {
    await (
      await caches.open("unrelated-app-cache")
    ).put("/unrelated", new Response("retain"));
  });
  await expect
    .poll(() =>
      page.evaluate(
        async () =>
          !(await navigator.serviceWorker.getRegistration("/"))?.installing,
      ),
    )
    .toBe(true);
  await page.getByRole("button", { name: "Delete offline cache" }).click();
  await expect(
    page.getByText("Offline cache is disabled on this device."),
  ).toBeVisible();
  await expect
    .poll(() =>
      page.evaluate(
        async () => (await navigator.serviceWorker.getRegistrations()).length,
      ),
    )
    .toBe(0);
  expect(await page.evaluate(() => caches.keys())).toEqual([
    "unrelated-app-cache",
  ]);
  expect(
    await other.evaluate(() => localStorage.getItem("liar.cache.disabled.v1")),
  ).toBe("true");
  await page.reload();
  await expect(
    page.getByText("Offline cache is disabled on this device."),
  ).toBeVisible();
  await page.getByRole("button", { name: "Prepare offline cache" }).click();
  await expect(
    page.getByText("Public local play assets are cached on this device."),
  ).toBeVisible();
  await context.route("**/api/cache-probe", (route) =>
    route.fulfill({ status: 200, body: '{"private":"do-not-cache"}' }),
  );
  await page.evaluate(() =>
    fetch("/api/cache-probe", { credentials: "include" }).then((reply) =>
      reply.text(),
    ),
  );
  const privateEntries = await page.evaluate(async () =>
    (
      await Promise.all(
        (await caches.keys()).map(async (key) =>
          (await (await caches.open(key)).keys()).map((row) => row.url),
        ),
      )
    )
      .flat()
      .filter((url) => url.includes("/api/")),
  );
  expect(privateEntries).toEqual([]);
  await other.close();
});
test("a fresh uncached offline navigation fails explicitly; denied SW registration keeps local play operable", async ({
  page,
  context,
}) => {
  await context.setOffline(true);
  await expect(page.goto("/en/daily")).rejects.toThrow();
  await context.setOffline(false);
  await page.addInitScript(() => {
    navigator.serviceWorker.register = () => Promise.reject(Error("denied"));
  });
  await page.goto("/en/daily");
  await expect(page.getByRole("gridcell")).toHaveCount(256);
  await expect(
    page.getByText(
      "The operation could not finish. Another tab may be playing, or storage may be unavailable. Try again from home.",
    ),
  ).toBeVisible();
  expect(
    await page.evaluate(() => navigator.serviceWorker.controller),
  ).toBeNull();
});
