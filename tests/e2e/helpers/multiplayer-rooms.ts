import { expect, test, type BrowserContext, type Page } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { advance } from "./online-quick";

const origin = "https://localhost:8443";

export function registerMultiplayerRoomTests(
  approve: (context: BrowserContext) => Promise<void>,
  login: (page: Page) => Promise<void>,
) {
  test("multiplayer invitation, explicit join, private board and standings survive a refresh", async ({
    browser,
  }) => {
    const one = await browser.newContext({
      baseURL: origin,
      ignoreHTTPSErrors: true,
      viewport: { width: 390, height: 844 },
    });
    const two = await browser.newContext({
      baseURL: origin,
      ignoreHTTPSErrors: true,
      viewport: { width: 360, height: 844 },
    });
    try {
      await approve(one);
      await approve(two);
      const host = await one.newPage(),
        guest = await two.newPage();
      await login(host);
      await host.goto("/en/rooms");
      await host.getByLabel("Room name").fill("Private race");
      await host.getByLabel("Visibility").selectOption("invite");
      await host
        .getByRole("button", { name: "Create room", exact: true })
        .click();
      await expect(
        host.getByRole("heading", { name: "Private race" }),
      ).toBeVisible();
      const code = await host
        .getByTestId("multiplayer-room-code")
        .textContent();
      expect(code).toMatch(/^[ABCDEFGHJKLMNPQRSTUVWXYZ23456789]{8}$/);
      await guest.goto(`/en/rooms?code=${code}`);
      await expect(
        guest.getByRole("heading", { name: "Multiplayer rooms" }),
      ).toBeVisible();
      await expect(guest.locator('meta[name="referrer"]')).toHaveAttribute(
        "content",
        "no-referrer",
      );
      await expect(guest.locator('script[src*="adsbygoogle"]')).toHaveCount(0);
      await expect(
        guest.getByText("Private race", { exact: true }),
      ).toHaveCount(0);
      await guest.getByLabel("Room code").press("Enter");
      await expect(guest).toHaveURL(/\/en\/login\?/);
      await guest.getByRole("button", { name: "Continue with Apple" }).click();
      await expect(guest).toHaveURL(
        `${origin}/en/onboarding?return_path=rooms&code=${code}`,
      );
      await guest
        .getByRole("textbox", { name: "Nickname" })
        .fill("Guest Player");
      await guest
        .getByRole("button", { name: "Save and start tutorial" })
        .click();
      await guest
        .getByRole("button", { name: "Skip tutorial", exact: true })
        .click();
      await expect(guest).toHaveURL(`${origin}/en/rooms?code=${code}`);
      await expect(guest.getByLabel("Room code")).toHaveValue(code!);
      await expect(host.getByText("Guest Player", { exact: true })).toHaveCount(
        0,
      );
      await guest
        .getByRole("button", { name: "Join room", exact: true })
        .click();
      await expect(
        guest.getByRole("heading", { name: "Private race" }),
      ).toBeVisible();
      await expect(
        host.getByText("Guest Player", { exact: true }),
      ).toBeVisible();
      await host.getByRole("button", { name: "Ready", exact: true }).click();
      await expect(
        guest
          .locator(".rooms-row")
          .filter({ hasText: "é探偵" })
          .getByText("Ready", { exact: true }),
      ).toBeVisible();
      await guest.getByRole("button", { name: "Ready", exact: true }).click();
      await host
        .getByRole("button", { name: "Start round", exact: true })
        .click();
      for (const page of [host, guest])
        await expect(page.getByRole("gridcell")).toHaveCount(9);
      await advance(host, 3);
      await expect(host.getByText(/^Time remaining /)).toBeVisible();
      const takeover = await one.newPage();
      await takeover.goto("/en/rooms");
      await expect(
        takeover.getByText(/^The match connection ended/),
      ).toBeVisible();
      await expect(takeover.getByRole("gridcell")).toHaveCount(0);
      await expect(host.getByRole("gridcell")).toHaveCount(9);
      await host.close();
      await expect(takeover.getByRole("gridcell")).toHaveCount(9, {
        timeout: 12000,
      });
      for (const cell of [8, 2, 6])
        await takeover.locator(`[data-cell="${cell}"]`).click();
      for (const page of [takeover, guest]) {
        await expect(
          page.getByRole("heading", { name: "Results", exact: true }),
        ).toBeVisible();
        await expect(page.getByText("Rank 1", { exact: true })).toBeVisible();
        await expect(page.locator("a[download]")).toHaveCount(0);
      }
      await guest.reload();
      await expect(guest.getByText("Rank 2", { exact: true })).toBeVisible();
      await expect(
        guest.getByRole("heading", { name: "Private race" }),
      ).toBeVisible();
      await expect(
        guest.getByRole("button", { name: "Ready", exact: true }),
      ).toBeEnabled();
    } finally {
      await Promise.all([one.close(), two.close()]);
    }
  });

  test("multiplayer public lobby headings fit all locales at phone and desktop widths", async ({
    page,
  }) => {
    for (const [index, locale] of [
      "en",
      "ko",
      "ja",
      "zh-CN",
      "es",
      "pt-BR",
      "de",
      "fr",
    ].entries()) {
      await page.setViewportSize({
        width: index % 3 === 0 ? 1440 : index % 2 ? 390 : 360,
        height: 900,
      });
      const copy = JSON.parse(
        await readFile(
          new URL(
            `../../../apps/web/src/locales/${locale}/common.json`,
            import.meta.url,
          ),
          "utf8",
        ),
      );
      await page.goto(`/${locale}/rooms`);
      await expect(
        page.getByRole("heading", { name: copy["rooms.title"] }),
      ).toBeVisible();
      await expect(
        page.getByRole("heading", { name: copy["rooms.publicList"] }),
      ).toBeVisible();
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= window.innerWidth,
        ),
      ).toBe(true);
    }
  });
}
