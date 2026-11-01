import { test, expect, type BrowserContext, type Page } from "@playwright/test";
import { mkdir, readFile } from "node:fs/promises";
import type {
  MeHistory,
  AuthBootstrap,
} from "../../../packages/protocol/src/index";
import { advance, stream } from "./online-quick";
type Approve = (context: BrowserContext) => Promise<void>;
type Login = (page: Page) => Promise<void>;
const origin = "https://localhost:8443";
export function registerMyInfoTests(approve: Approve, login: Login) {
  for (const width of [1440, 360])
    test(`my info shows own online history with no personal downloads in eight languages at ${width}px`, async ({
      browser,
    }) => {
      const a = await browser.newContext({
          baseURL: origin,
          ignoreHTTPSErrors: true,
          viewport: { width, height: 900 },
        }),
        b = await browser.newContext({
          baseURL: origin,
          ignoreHTTPSErrors: true,
        });
      try {
        await approve(a);
        await approve(b);
        const one = await a.newPage(),
          two = await b.newPage();
        await login(one);
        await login(two);
        const observed = stream(one),
          peer = stream(two);
        // Exercise the peer connecting after the first seat's snapshot.
        await two.routeWebSocket("**/api/v1/ws", async (client) => {
          await expect.poll(() => Boolean(observed.snapshot())).toBe(true);
          client.connectToServer();
        });
        await one.goto("/en/queue");
        await one.getByRole("button", { name: "Find an opponent" }).click();
        await expect(
          one.getByRole("button", { name: "Cancel waiting" }),
        ).toBeVisible();
        await two.goto("/en/queue");
        await two.getByRole("button", { name: "Find an opponent" }).click();
        await expect(one.getByRole("gridcell")).toHaveCount(9);
        await expect(two.getByRole("gridcell")).toHaveCount(9);
        await expect
          .poll(() => Boolean(observed.snapshot() && peer.snapshot()))
          .toBe(true);
        expect(peer.snapshot()!.match_id).toBe(observed.snapshot()!.match_id);
        await advance(one, 3);
        await expect.poll(() => observed.lastView()?.phase).toBe("playing");
        await expect.poll(() => peer.lastView()?.phase).toBe("playing");
        await one.locator('[data-cell="8"]').click();
        await expect
          .poll(() => observed.lastView()?.own.cells[8].state)
          .toBe("safe");
        await advance(one, 240);
        await expect(one.getByTestId("recording")).toHaveText("Result saved.");
        await expect(two.getByTestId("recording")).toHaveText("Result saved.");
        const matchId = observed.snapshot()!.match_id,
          ownOpened = observed
            .lastView()!
            .own.cells.filter((c) => c.state === "safe").length;
        expect(
          peer.lastView()!.own.cells.filter((c) => c.state === "safe"),
        ).toHaveLength(4);
        if (width === 360) {
          await one.goto("/en/me/account");
          await one
            .getByRole("button", { name: "Change nickname", exact: true })
            .click();
          await one
            .getByRole("textbox", { name: "Nickname", exact: true })
            .fill("W".repeat(16));
          await one
            .getByRole("textbox", { name: "Nickname", exact: true })
            .press("Enter");
          await expect(
            one.getByText("W".repeat(16), { exact: true }),
          ).toBeVisible();
        }
        let downloads = 0;
        one.on("download", () => downloads++);
        for (const locale of [
          "en",
          "ko",
          "ja",
          "zh-CN",
          "es",
          "pt-BR",
          "de",
          "fr",
        ]) {
          await advance(one, 1);
          const messages = JSON.parse(
            await readFile(
              new URL(
                `../../../apps/web/src/locales/${locale}/common.json`,
                import.meta.url,
              ),
              "utf8",
            ),
          ) as Record<string, string>;
          const capture = async (screen: string) => {
            expect(
              await one.evaluate(
                () => document.documentElement.scrollWidth <= innerWidth,
              ),
              `${locale}/${screen} must fit ${width}px`,
            ).toBe(true);
            if (locale !== "ko") return;
            await mkdir(".tmp/my-info-layout", { recursive: true });
            await one.screenshot({
              path: `.tmp/my-info-layout/${screen}-${width}.png`,
              fullPage: true,
            });
          };
          await one.goto(`/${locale}/`);
          const entry = one
            .locator("header")
            .getByRole("link", { name: messages["me.title"], exact: true });
          await entry.focus();
          await one.keyboard.press("Enter");
          await expect(
            one.getByRole("heading", { name: messages["me.title"], level: 1 }),
          ).toBeVisible();
          await expect(one.getByTestId("me-summary")).toBeVisible();
          await capture("overview");
          await expect(one.locator('meta[name="robots"]')).toHaveAttribute(
            "content",
            /noindex/,
          );
          const tabs = one.getByRole("navigation", {
            name: messages["me.title"],
            exact: true,
          });
          const received = one.waitForResponse(
            (response) =>
              new URL(response.url()).pathname === "/api/v1/me/history" &&
              response.request().method() === "POST",
          );
          const historyLink = tabs.getByRole("link", {
            name: messages["me.history"],
            exact: true,
          });
          await historyLink.focus();
          await one.keyboard.press("Enter");
          const response = await received;
          expect(response.status()).toBe(200);
          expect(response.headers()["cache-control"]).toBe("no-store");
          const value = (await response.json()) as MeHistory;
          expect(value.total).toBe(1);
          expect(value.items).toHaveLength(1);
          expect(value.items[0].kind).toBe("online");
          const record = value.items[0];
          if (record.kind !== "online") throw Error("Missing online record");
          expect(record.entry.result.match_id).toBe(matchId);
          expect(record.entry.result.own?.opened_safe).toBe(ownOpened);
          expect(record.entry.opponent).toBe("human");
          await expect(one.getByTestId("me-record")).toHaveCount(1);
          await one.getByTestId("me-record").locator("summary").press("Enter");
          await expect(one.getByTestId("personal-result")).toBeVisible();
          await capture("history");
          await one
            .getByRole("combobox", {
              name: messages["me.opponent"],
              exact: true,
            })
            .selectOption("bot");
          await expect(
            one.getByText(messages["me.empty"], { exact: true }),
          ).toBeVisible();
          await expect(one.getByTestId("me-win-rate")).toHaveText("—");
          await tabs
            .getByRole("link", { name: messages["me.account"], exact: true })
            .click();
          await expect(
            one.getByRole("button", {
              name: messages["auth.changeNickname"],
              exact: true,
            }),
          ).toBeVisible();
          await expect(
            one.getByText(messages["me.created"], { exact: true }),
          ).toBeVisible();
          await capture("account");
          await tabs
            .getByRole("link", { name: messages["me.privacy"], exact: true })
            .click();
          await expect(one.getByTestId("me-server-consent")).toContainText(
            messages["me.cmpUnavailable"],
          );
          await expect(
            one.getByRole("button", {
              name: messages["consent.reopen"],
              exact: true,
            }),
          ).toBeDisabled();
          expect(
            await one.evaluate(
              () => document.documentElement.scrollWidth <= innerWidth,
            ),
          ).toBe(true);
          await capture("privacy");
          await tabs
            .getByRole("link", { name: messages["settings"], exact: true })
            .click();
          await expect(
            one.getByText(messages["me.deviceOnly"], { exact: true }),
          ).toBeVisible();
          await expect(one.locator('meta[name="robots"]')).toHaveCount(0);
          expect(JSON.stringify(value)).not.toMatch(
            /secret|seed|board|subject|digest|credential|csrf|session_revision|token|password|email/,
          );
          const proof = (await one.request
            .get("/api/v1/auth/bootstrap")
            .then((r) => r.json())) as AuthBootstrap;
          for (const path of [
            "/api/v1/me/export",
            "/api/v1/me/games/export",
            "/api/v1/me/games/replay",
          ]) {
            expect(
              (
                await one.request.post(path, {
                  headers: { Origin: origin, "x-liar-csrf": proof.csrf! },
                  data: {},
                })
              ).status(),
            ).toBe(404);
          }
          const stored = await one.evaluate(() =>
            JSON.stringify({ ...localStorage, ...sessionStorage }),
          );
          expect(stored).not.toContain(matchId);
          expect(stored).not.toContain(value.account_id);
          expect(downloads).toBe(0);
        }
      } finally {
        await a.close();
        await b.close();
      }
    });
  test("my info works before nickname setup and discards a held private response after peer sign-out", async ({
    browser,
  }) => {
    const context = await browser.newContext({
      baseURL: origin,
      ignoreHTTPSErrors: true,
      viewport: { width: 360, height: 844 },
    });
    let release = () => {};
    try {
      await approve(context);
      const page = await context.newPage();
      await page.goto("/en/login?return_path=me");
      await page.getByRole("button", { name: "Continue with Google" }).click();
      await expect(page).toHaveURL(/\/en\/onboarding\?return_path=me/);
      const messages = JSON.parse(
        await readFile(
          new URL(
            "../../../apps/web/src/locales/ko/common.json",
            import.meta.url,
          ),
          "utf8",
        ),
      ) as Record<string, string>;
      await page.goto("/ko/me");
      await expect(page.getByTestId("me-summary")).toBeVisible();
      const proof = (await page.request
        .get("/api/v1/auth/bootstrap")
        .then((r) => r.json())) as AuthBootstrap;
      expect(proof.account?.nickname).toBeNull();
      await advance(page, 301);
      await page.goto("/ko/me/account");
      await expect(
        page.getByRole("button", {
          name: messages["auth.delete"],
          exact: true,
        }),
      ).toBeEnabled();
      await expect(
        page.getByText(messages["me.created"], { exact: true }),
      ).toBeVisible();
      const peer = await context.newPage();
      await peer.goto("/en/me/account");
      await expect(
        peer.getByRole("button", { name: "Sign out", exact: true }),
      ).toBeEnabled();
      let captured!: () => void;
      const received = new Promise<void>((resolve) => {
        captured = resolve;
      });
      const hold = new Promise<void>((resolve) => {
        release = resolve;
      });
      let finished!: () => void;
      const done = new Promise<void>((resolve) => {
        finished = resolve;
      });
      await page.route(
        "**/api/v1/me/overview",
        async (route) => {
          try {
            const response = await route.fetch();
            expect(response.status()).toBe(200);
            captured();
            await hold;
            try {
              await route.fulfill({ response });
            } catch (error) {
              if (!route.request().failure()) throw error;
            }
          } finally {
            finished();
          }
        },
        { times: 1 },
      );
      await page.goto("/ko/me");
      await received;
      await peer.getByRole("button", { name: "Sign out", exact: true }).click();
      await expect(
        page.getByText(messages["me.signIn"], { exact: true }),
      ).toBeVisible();
      await expect(page.getByTestId("me-summary")).toHaveCount(0);
      release();
      await done;
      await expect(page.getByTestId("me-summary")).toHaveCount(0);
      expect(
        (await page.request.get("/api/v1/auth/bootstrap").then((r) => r.json()))
          .account,
      ).toBeNull();
      await page.goto("/ko/settings");
      await expect(
        page.getByRole("button", {
          name: messages["consent.reopen"],
          exact: true,
        }),
      ).toBeDisabled();
      expect(
        (await page.request.get("/api/v1/auth/bootstrap").then((r) => r.json()))
          .account,
      ).toBeNull();
    } finally {
      release();
      await context.close();
    }
  });
}
