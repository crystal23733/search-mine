import {
  test,
  expect,
  type Browser,
  type BrowserContext,
  type Page,
} from "@playwright/test";
import { mkdir } from "node:fs/promises";
import { advance, stream } from "./online-quick";
const origin = "https://localhost:8443";
type Approve = (context: BrowserContext) => Promise<void>;
type Login = (page: Page) => Promise<void>;
async function context(browser: Browser, width = 1440) {
  return browser.newContext({
    baseURL: origin,
    ignoreHTTPSErrors: true,
    viewport: { width, height: width === 390 ? 844 : 900 },
  });
}
export function registerOnlineFriendsTests(approve: Approve, login: Login) {
  for (const width of [1440, 390])
    test(`actual friend invitation, explicit OAuth join, ready and same-board result at ${width}px`, async ({
      browser,
    }) => {
      const a = await context(browser, width),
        b = await context(browser, width),
        c = await context(browser);
      try {
        await approve(a);
        await approve(b);
        await approve(c);
        await a.grantPermissions(["clipboard-read", "clipboard-write"], {
          origin,
        });
        const one = await a.newPage(),
          two = await b.newPage(),
          third = await c.newPage();
        await login(one);
        await login(third);
        const first = stream(one),
          second = stream(two);
        const requests: string[] = [];
        one.on("request", (r) => requests.push(r.url()));
        await one.goto("/en/friends");
        await one
          .getByRole("button", { name: "Create room", exact: true })
          .click();
        const code = await one.getByTestId("room-code").textContent();
        expect(code).toMatch(/^[ABCDEFGHJKLMNPQRSTUVWXYZ23456789]{8}$/);
        await expect(
          one.getByRole("heading", { name: "Seats 1/2" }),
        ).toBeVisible();
        await one.getByRole("button", { name: "Copy invitation link" }).click();
        const invitation = await one.evaluate(() =>
          navigator.clipboard.readText(),
        );
        expect(invitation).toBe(`${origin}/en/friends?code=${code}`);
        await two.goto(invitation);
        await two
          .getByRole("button", { name: "Skip tutorial", exact: true })
          .click();
        await expect(two).toHaveURL(invitation);
        await two
          .getByRole("link", {
            name: "Use an account you already have",
            exact: true,
          })
          .click();
        await expect(two).toHaveURL(
          `${origin}/en/login?code=${code}&return_path=friends`,
        );
        await two
          .getByRole("button", {
            name: `Continue with ${width === 390 ? "Apple" : "Google"}`,
          })
          .click();
        await expect(two).toHaveURL(
          `${origin}/en/onboarding?return_path=friends&code=${code}`,
        );
        await two
          .getByRole("textbox", { name: "Nickname" })
          .fill(`Friend${width}`);
        await two
          .getByRole("button", { name: "Save and start tutorial" })
          .click();
        await two
          .getByRole("button", { name: "Skip tutorial", exact: true })
          .click();
        await expect(two).toHaveURL(invitation);
        await expect(
          two.getByRole("textbox", { name: "Room code" }),
        ).toHaveValue(code!);
        await expect(
          one.getByRole("heading", { name: "Seats 1/2" }),
        ).toBeVisible();
        expect(second.snapshot()).toBeUndefined();
        await two
          .getByRole("button", { name: "Join room", exact: true })
          .click();
        await expect(two.getByTestId("room-code")).toHaveText(code!);
        await expect(
          one.getByRole("heading", { name: "Seats 2/2" }),
        ).toBeVisible();
        await third.goto(invitation);
        await third
          .getByRole("button", { name: "Join room", exact: true })
          .click();
        await expect(third.getByRole("alert")).toHaveText(
          "This room is full. Ask your friend for another invitation.",
        );
        await expect(third.getByRole("grid")).toHaveCount(0);
        await mkdir(".tmp/online-layout", { recursive: true });
        await one.screenshot({
          path: `.tmp/online-layout/friends-${width}.png`,
          fullPage: true,
        });
        for (const locale of [
          "ko",
          "ja",
          "zh-CN",
          "es",
          "pt-BR",
          "de",
          "fr",
          "en",
        ]) {
          await one.locator("select").first().selectOption(locale);
          await expect(one.locator("html")).toHaveAttribute("lang", locale);
          await expect(one.getByTestId("room-code")).toHaveText(code!);
          expect(
            await one.evaluate(
              () => document.documentElement.scrollWidth <= innerWidth,
            ),
          ).toBe(true);
        }
        await one.getByRole("button", { name: "Ready", exact: true }).click();
        await expect(
          one.getByRole("button", { name: "Not ready", exact: true }),
        ).toHaveAttribute("aria-pressed", "true");
        await one
          .getByRole("button", { name: "Not ready", exact: true })
          .click();
        await expect(
          one.getByRole("button", { name: "Ready", exact: true }),
        ).toHaveAttribute("aria-pressed", "false");
        expect(first.snapshot()).toBeUndefined();
        expect(second.snapshot()).toBeUndefined();
        await one.getByRole("button", { name: "Ready", exact: true }).click();
        await two.getByRole("button", { name: "Ready", exact: true }).click();
        await expect(one.getByRole("gridcell")).toHaveCount(9);
        await expect(two.getByRole("gridcell")).toHaveCount(9);
        expect(first.snapshot()!.match_id).toBe(second.snapshot()!.match_id);
        expect(first.lastView()!.own.cells).toEqual(
          second.lastView()!.own.cells,
        );
        await advance(one, 3);
        await expect.poll(() => first.lastView()?.phase).toBe("playing");
        await one.locator('[data-cell="2"]').click();
        await expect
          .poll(() => first.lastView()?.own.cells[2].state)
          .toBe("safe");
        await advance(one, 240);
        await expect(one.getByTestId("recording")).toHaveText("Result saved.");
        await expect(two.getByTestId("recording")).toHaveText("Result saved.");
        expect(JSON.stringify(first.events)).not.toMatch(
          /"(?:seed|truth|credential|csrf|account_id|session_revision)"/,
        );
        expect(
          requests.filter((url) =>
            /doubleclick|googlesyndication|googleadservices/.test(url),
          ),
        ).toEqual([]);
        const stored = await one.evaluate(() =>
          [
            ...Array.from({ length: localStorage.length }, (_, i) =>
              localStorage.getItem(localStorage.key(i)!),
            ),
            ...Array.from({ length: sessionStorage.length }, (_, i) =>
              sessionStorage.getItem(sessionStorage.key(i)!),
            ),
          ].join(" "),
        );
        expect(stored).not.toContain(code!);
        expect(stored).not.toContain(first.snapshot()!.match_id);
        await one
          .getByRole("button", { name: "Back to friends lobby", exact: true })
          .click();
        await expect(
          one.getByRole("button", { name: "Create room", exact: true }),
        ).toBeVisible();
      } finally {
        await a.close();
        await b.close();
        await c.close();
      }
    });
  test("actual host leave promotes the remaining seat and server expiry rejects the old invitation", async ({
    browser,
  }) => {
    const a = await context(browser),
      b = await context(browser, 390);
    try {
      await approve(a);
      await approve(b);
      const one = await a.newPage(),
        two = await b.newPage();
      await login(one);
      await login(two);
      await one.goto("/en/friends");
      await one
        .getByRole("button", { name: "Create room", exact: true })
        .click();
      const code = (await one.getByTestId("room-code").textContent())!;
      await one.getByRole("button", { name: "Ready", exact: true }).click();
      await expect(
        one.getByRole("button", { name: "Not ready", exact: true }),
      ).toHaveAttribute("aria-pressed", "true");
      await two.goto(`/en/friends?code=${code.toLowerCase()}`);
      await expect(two.getByRole("textbox", { name: "Room code" })).toHaveValue(
        code,
      );
      await two.getByRole("button", { name: "Join room", exact: true }).click();
      await expect(two.getByTestId("room-seat-1")).toContainText("You");
      await one
        .getByRole("button", { name: "Leave room", exact: true })
        .click();
      await expect(
        one.getByRole("button", { name: "Create room", exact: true }),
      ).toBeVisible();
      await expect(two.getByTestId("room-seat-0")).toContainText("You");
      await expect(
        two.getByRole("heading", { name: "Seats 1/2" }),
      ).toBeVisible();
      await expect(
        two.getByRole("button", { name: "Ready", exact: true }),
      ).toHaveAttribute("aria-pressed", "false");
      await advance(two, 601);
      await expect(
        two.getByRole("button", { name: "Create room", exact: true }),
      ).toBeVisible();
      await expect(
        two.getByText(
          "This room is no longer active for you. Create or join another room.",
        ),
      ).toBeVisible();
      await one.getByRole("textbox", { name: "Room code" }).fill(code);
      await one.getByRole("button", { name: "Join room", exact: true }).click();
      await expect(one.getByRole("alert")).toHaveText(
        "This room is unavailable or has expired. Check the code or create a new room.",
      );
      await one.getByRole("button", { name: "Check match status" }).click();
      await one.getByRole("textbox", { name: "Room code" }).fill("ABCD2340");
      await expect(
        one.getByRole("button", { name: "Join room", exact: true }),
      ).toBeDisabled();
      await expect(
        one.getByRole("textbox", { name: "Room code" }),
      ).toHaveAttribute("aria-invalid", "true");
      const bootstrap = await one.request
        .get("/api/v1/auth/bootstrap")
        .then((r) => r.json());
      const statuses: number[] = [];
      for (let i = 0; i < 10; i++)
        statuses.push(
          (
            await one.request.post("/api/v1/lobby", {
              headers: { Origin: origin, "x-liar-csrf": bootstrap.csrf },
              data: { v: 1, command: { type: "room_join", code } },
            })
          ).status(),
        );
      expect(statuses).toContain(429);
      await one.getByRole("textbox", { name: "Room code" }).fill(code);
      await one.getByRole("button", { name: "Join room", exact: true }).click();
      await expect(one.getByRole("alert")).toHaveText(
        "The service is busy. Wait briefly and check again.",
      );
    } finally {
      await a.close();
      await b.close();
    }
  });
}
