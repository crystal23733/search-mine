import { test, expect } from "@playwright/test";
const locales = ["en", "ko", "ja", "zh-CN", "es", "pt-BR", "de", "fr"];
export function pwaInstallationTests() {
  test("production PWA metadata and real icon responses support installation without account URLs", async ({
    page,
  }) => {
    await page.goto("/en/settings");
    const manifestLink = page.locator('link[rel="manifest"]');
    await expect(manifestLink).toHaveCount(1);
    const url = new URL((await manifestLink.getAttribute("href"))!, page.url());
    expect(url.origin).toBe(new URL(page.url()).origin);
    const response = await page.request.get(url.href);
    expect(response.status()).toBe(200);
    expect(response.headers()["content-type"]).toContain(
      "application/manifest+json",
    );
    const manifest = await response.json();
    expect(manifest).toMatchObject({
      id: "/",
      scope: "/",
      start_url: "/",
      display: "standalone",
      prefer_related_applications: false,
    });
    expect(
      manifest.icons.some(
        (icon: { sizes: string; purpose: string }) =>
          icon.sizes === "192x192" && icon.purpose === "any",
      ),
    ).toBe(true);
    expect(
      manifest.icons.some(
        (icon: { sizes: string; purpose: string }) =>
          icon.sizes === "512x512" && icon.purpose === "maskable",
      ),
    ).toBe(true);
    for (const icon of [
      ...manifest.icons,
      { src: "/icons/apple-touch-icon-180x180.png", sizes: "180x180" },
    ]) {
      const image = await page.request.get(new URL(icon.src, url).href);
      expect(image.status()).toBe(200);
      expect(image.headers()["content-type"]).toContain("image/png");
      const bytes = await image.body();
      expect(bytes.subarray(0, 8).toString("hex")).toBe("89504e470d0a1a0a");
      const [width, height] = icon.sizes.split("x").map(Number);
      expect([bytes.readUInt32BE(16), bytes.readUInt32BE(20)]).toEqual([
        width,
        height,
      ]);
    }
    for (const locale of locales)
      for (const route of ["/", "/rules", "/learn", "/storage"]) {
        const document = await page.request.get(`/${locale}${route}`);
        expect(document.status()).toBe(200);
        expect(await document.text()).toContain('rel="manifest"');
      }
    const session = await page.context().newCDPSession(page);
    try {
      await session.send("Page.enable");
      const result = await session.send("Page.getAppManifest");
      expect(result.url).toBe(url.href);
      expect(result.errors.filter((error) => error.critical)).toEqual([]);
    } finally {
      await session.detach();
    }
  });
  test("simulated install invitation is optional, keyboard-triggered and cancellable in eight languages without downloads", async ({
    page,
  }) => {
    test.setTimeout(60_000);
    // Isolate the synthetic lifecycle from an unsolicited native OS dialog.
    await page.addInitScript(() => {
      window.addEventListener(
        "beforeinstallprompt",
        (event) => {
          if (event.isTrusted) {
            event.preventDefault();
            event.stopImmediatePropagation();
          }
        },
        { capture: true },
      );
    });
    let downloads = 0;
    page.on("download", () => downloads++);
    for (const locale of locales) {
      await page.goto(`/${locale}/settings`);
      const card = page.locator('section[aria-labelledby="install-title"]');
      await expect(card.getByRole("heading")).toBeVisible();
      await expect(card.getByRole("button")).toHaveCount(0);
      const prevented = await page.evaluate(() => {
        const observed = { calls: 0, active: false };
        Object.assign(window, { __installContract: observed });
        const event = Object.assign(
          new Event("beforeinstallprompt", { cancelable: true }),
          {
            prompt: async () => {
              observed.calls++;
              observed.active = navigator.userActivation.isActive;
            },
            userChoice: Promise.resolve({ outcome: "dismissed" }),
          },
        );
        window.dispatchEvent(event);
        return event.defaultPrevented;
      });
      expect(prevented).toBe(true);
      expect(
        await page.evaluate(
          () =>
            (window as unknown as { __installContract: { calls: number } })
              .__installContract.calls,
        ),
      ).toBe(0);
      await card.getByRole("button").press("Enter");
      await expect(card.getByRole("button")).toHaveCount(0);
      await expect(card.getByRole("status")).toHaveCount(2);
      expect(
        await page.evaluate(
          () =>
            (
              window as unknown as {
                __installContract: { calls: number; active: boolean };
              }
            ).__installContract,
        ),
      ).toEqual({ calls: 1, active: true });
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= window.innerWidth,
        ),
      ).toBe(true);
    }
    expect(downloads).toBe(0);
  });
}
