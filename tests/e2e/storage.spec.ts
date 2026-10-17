import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";
import { unrequestedServiceCall } from "./network";

const locales = ["en", "ko", "ja", "zh-CN", "es", "pt-BR", "de", "fr"];
const messages = (locale: string): Record<string, string> =>
  JSON.parse(
    readFileSync(`apps/web/src/locales/${locale}/common.json`, "utf8"),
  );

test("unconfigured consent ignores delivered peer grants and invalidation while local play remains available", async ({
  page,
  context,
}) => {
  const requests: string[] = [];
  context.on("request", (request) => requests.push(request.url()));
  await context.addInitScript(() =>
    localStorage.setItem("liar.tutorial.v1", '"skipped"'),
  );
  await page.goto("/en/settings");
  await page.evaluate(() => {
    const probe = {
      channel: new BroadcastChannel("liar.consent.v1"),
      received: 0,
    };
    probe.channel.onmessage = () => {
      probe.received++;
    };
    (window as Window & { consentTestProbe?: typeof probe }).consentTestProbe =
      probe;
  });
  const peer = await context.newPage();
  await peer.goto("/en/settings");
  await peer.evaluate(() => {
    const channel = new BroadcastChannel("liar.consent.v1");
    channel.postMessage({
      status: "valid",
      analytics: true,
      advertising: true,
      context: {},
      revision: 1,
      validFrom: 0,
      expiresAt: Number.MAX_SAFE_INTEGER,
    });
    channel.postMessage("invalidate");
    channel.close();
  });
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as Window & { consentTestProbe?: { received: number } })
            .consentTestProbe?.received,
      ),
    )
    .toBe(2);
  const consent = page.getByRole("region", { name: "Consent settings" });
  await expect(consent.getByRole("status")).toHaveText(
    messages("en")["consent.state.unavailable"],
  );
  await expect(consent.locator("dd")).toHaveText([
    messages("en")["consent.blocked"],
    messages("en")["consent.blocked"],
  ]);
  await expect(
    consent.getByRole("button", { name: "Reopen consent choices" }),
  ).toBeDisabled();
  await page.evaluate(() =>
    (
      window as Window & { consentTestProbe?: { channel: BroadcastChannel } }
    ).consentTestProbe?.channel.close(),
  );
  await peer.close();
  await page.getByRole("link", { name: "Liar Sweeper", exact: true }).click();
  await page
    .getByRole("link", { name: "Practise locally", exact: true })
    .click();
  await expect(page.getByRole("gridcell")).toHaveCount(256);
  expect(
    requests.filter((url) => unrequestedServiceCall(url, page.url())),
  ).toEqual([]);
});

test("storage purposes and clearing controls are readable in eight locales without optional requests", async ({
  page,
}, info) => {
  const requests: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  await page.addInitScript(() =>
    localStorage.setItem("liar.tutorial.v1", '"skipped"'),
  );
  await page.setViewportSize({
    width: info.project.name === "chromium" ? 1440 : 390,
    height: 800,
  });
  for (const locale of locales) {
    if (locale === "de")
      await page.setViewportSize({ width: 360, height: 800 });
    const text = messages(locale);
    await page.goto(`/${locale}/rules`);
    await page.locator(`footer a[href="/${locale}/storage"]`).click();
    await expect(page).toHaveURL(new RegExp(`/${locale}/storage$`));
    await expect(page.getByRole("heading", { level: 1 })).toHaveText(
      text["storage.pageTitle"],
    );
    for (const purpose of [
      "preferences",
      "records",
      "cache",
      "cookies",
      "optional",
    ]) {
      await expect(
        page
          .locator("main dt")
          .filter({ hasText: text[`storage.${purpose}.title`] }),
      ).toBeVisible();
      await expect(
        page
          .locator("main dd")
          .filter({ hasText: text[`storage.${purpose}.description`] }),
      ).toBeVisible();
    }
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    await page
      .locator("main")
      .getByRole("link", { name: text.settings, exact: true })
      .click();
    const consent = page.getByRole("region", { name: text.consent });
    await expect(consent.getByRole("status")).toHaveText(
      text["consent.state.unavailable"],
    );
    await expect(consent.locator("dd")).toHaveText([
      text["consent.blocked"],
      text["consent.blocked"],
    ]);
    await expect(
      consent.getByRole("button", {
        name: text["consent.reopen"],
        exact: true,
      }),
    ).toBeDisabled();
    const opener = page.getByRole("button", {
      name: text["storage.title"],
      exact: true,
    });
    await opener.click();
    const dialog = page.getByRole("dialog", { name: text["storage.title"] });
    await expect(
      dialog.getByText(text["storage.intro"], { exact: true }),
    ).toBeInViewport();
    await expect(
      dialog.getByText(text["storage.cookies.description"], { exact: true }),
    ).toBeVisible();
    await expect(
      dialog.getByText(text["storage.records.description"], { exact: true }),
    ).toBeVisible();
    await expect(
      dialog.getByRole("heading", { name: text["storage.title"] }),
    ).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(
      dialog.getByRole("button", { name: text.close, exact: true }),
    ).toBeInViewport();
    await page.keyboard.press("Tab");
    expect(
      await page.evaluate(() =>
        Boolean(document.activeElement?.closest("dialog")),
      ),
    ).toBe(true);
    await page.keyboard.press("Escape");
    await expect(dialog).not.toBeVisible();
    await expect(opener).toBeFocused();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
  }
  expect(
    requests.filter((url) => unrequestedServiceCall(url, page.url())),
  ).toEqual([]);
  expect(
    requests.filter(
      (url) => new URL(url).origin !== new URL(page.url()).origin,
    ),
  ).toEqual([]);
});

test("storage notice stays available with denied storage and failed authentication; local practice still starts", async ({
  page,
}) => {
  const requests: string[] = [];
  let bootstrapFailures = 0;
  page.on("request", (request) => requests.push(request.url()));
  await page.addInitScript(() => {
    for (const name of ["localStorage", "indexedDB"])
      Object.defineProperty(window, name, {
        get() {
          throw Error("storage_denied");
        },
      });
  });
  await page.route("**/api/v1/auth/bootstrap", (route) => {
    bootstrapFailures++;
    return route.fulfill({ status: 503, json: { code: "auth_unavailable" } });
  });
  await page.goto("/en/storage");
  await expect(page.getByRole("heading", { level: 1 })).toHaveText(
    "Storage and cookies",
  );
  await expect(
    page.getByText(messages("en")["storage.fallback"], { exact: true }),
  ).toBeVisible();
  await page
    .locator("main")
    .getByRole("link", { name: "Settings", exact: true })
    .click();
  await expect(
    page.getByRole("region", { name: "Consent settings" }).getByRole("status"),
  ).toHaveText(messages("en")["consent.state.unavailable"]);
  await page.getByRole("link", { name: "Liar Sweeper", exact: true }).click();
  await page
    .getByRole("button", { name: "Skip tutorial", exact: true })
    .click();
  await page
    .getByRole("link", { name: "Practise locally", exact: true })
    .click();
  await expect(page.getByRole("gridcell")).toHaveCount(256);
  expect(bootstrapFailures).toBeGreaterThan(0);
  expect(
    requests.filter((url) => unrequestedServiceCall(url, page.url())),
  ).toEqual([]);
});
