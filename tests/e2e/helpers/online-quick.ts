import { test, expect, type BrowserContext, type Page } from "@playwright/test";
import type {
  OnlineEvent,
  OnlineInput,
} from "../../../packages/protocol/src/index";
import { mkdir } from "node:fs/promises";
const origin = "https://localhost:8443";
type Approve = (context: BrowserContext) => Promise<void>;
type Login = (page: Page) => Promise<void>;
export function stream(page: Page) {
  const events: OnlineEvent[] = [];
  const commands: OnlineInput[] = [];
  page.on("websocket", (socket) => {
    expect(socket.url()).toBe(`${origin.replace("https:", "wss:")}/api/v1/ws`);
    socket.on("framereceived", (frame) => {
      if (typeof frame.payload === "string")
        events.push(JSON.parse(frame.payload));
    });
    socket.on("framesent", (frame) => {
      if (typeof frame.payload === "string" && frame.payload.startsWith("{"))
        commands.push(JSON.parse(frame.payload));
    });
  });
  return {
    events,
    commands,
    snapshot: () => events.find((e) => e.payload.type === "snapshot"),
    lastView: () =>
      events
        .flatMap((e) => ("view" in e.payload ? [e.payload.view] : []))
        .at(-1),
  };
}
export async function advance(page: Page, seconds: number) {
  expect(
    (await page.request.post("/__fixture/advance", { data: { seconds } })).ok(),
  ).toBe(true);
}
function cell(page: Page, index: number) {
  return page.locator(`[data-cell="${index}"]`);
}
export function registerOnlineQuickTests(approve: Approve, login: Login) {
  for (const width of [1440, 390])
    test(`actual quick match has the same private-backed board, inputs and SQL result at ${width}px`, async ({
      browser,
    }) => {
      const a = await browser.newContext({
          baseURL: origin,
          ignoreHTTPSErrors: true,
          viewport: { width, height: width === 390 ? 844 : 900 },
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
        const first = stream(one),
          second = stream(two);
        const requests: string[] = [];
        one.on("request", (r) => requests.push(r.url()));
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
          .poll(() => Boolean(first.snapshot() && second.snapshot()))
          .toBe(true);
        const initialOne = first.lastView()!,
          initialTwo = second.lastView()!;
        expect(initialOne.own.cells).toEqual(initialTwo.own.cells);
        expect(first.snapshot()!.match_id).toBe(second.snapshot()!.match_id);
        expect(
          initialOne.own.cells.filter((c) => c.state === "safe"),
        ).toHaveLength(4);
        await advance(one, 3);
        await expect.poll(() => first.lastView()?.phase).toBe("playing");
        await expect.poll(() => second.lastView()?.phase).toBe("playing");
        await cell(one, 2).click({ button: "right" });
        await expect(cell(one, 2)).toHaveAttribute("aria-label", /flag/i);
        await cell(one, 2).click({ button: "right" });
        await expect(cell(one, 2)).not.toHaveAttribute("aria-label", /flag/i);
        await cell(one, 8).click();
        await expect
          .poll(() => first.lastView()?.own.cells[8].state)
          .toBe("safe");
        const attack = one.getByRole("button", { name: "Attack", exact: true });
        await expect(attack).toBeEnabled();
        await attack.click();
        await expect
          .poll(() => first.commands.at(-1)?.action.type)
          .toBe("attack");
        const rejectedId = first.commands.at(-1)!.command_id;
        await expect
          .poll(() =>
            first.events.some(
              (e) =>
                e.payload.type === "ack" &&
                e.payload.command_id === rejectedId &&
                e.payload.status === "rejected" &&
                e.payload.error === "unavailable",
            ),
          )
          .toBe(true);
        expect(first.lastView()?.own.gauge).toBe(1);
        await cell(two, 8).click();
        await expect
          .poll(() => second.lastView()?.own.cells[8].state)
          .toBe("safe");
        await attack.click();
        await expect
          .poll(
            () =>
              first.commands.filter((c) => c.action.type === "attack").length,
          )
          .toBe(2);
        const acceptedId = first.commands.at(-1)!.command_id;
        await expect
          .poll(() =>
            first.events.some(
              (e) =>
                e.payload.type === "ack" &&
                e.payload.command_id === acceptedId &&
                e.payload.status === "applied",
            ),
          )
          .toBe(true);
        await expect.poll(() => first.lastView()?.own.gauge).toBe(0);
        await cell(one, 2).click();
        await expect
          .poll(() => first.lastView()?.own.cells[2].state)
          .toBe("safe");
        await cell(two, 2).click();
        await expect.poll(() => second.lastView()?.own.cells[2].number).toBe(2);
        await two.getByRole("button", { name: "Accuse", exact: true }).click();
        await cell(two, 2).click();
        await expect
          .poll(() => second.lastView()?.own.stats.correct_accusations)
          .toBe(1);
        expect(second.lastView()?.own.cells[2].number).toBe(1);
        await expect.poll(() => first.lastView()?.own.stun_ms).toBe(2000);
        await advance(one, 3);
        await one.getByRole("button", { name: "Accuse", exact: true }).click();
        await cell(one, 2).click();
        await expect
          .poll(() => first.lastView()?.own.stats.accusation_attempts)
          .toBe(1);
        await advance(one, 3);
        await one
          .getByRole("combobox", { name: "Language" })
          .selectOption("ko");
        await expect(one.locator("html")).toHaveAttribute("lang", "ko");
        expect(
          first.events.filter((e) => e.payload.type === "snapshot"),
        ).toHaveLength(1);
        await advance(one, 240);
        await expect(one.getByTestId("recording")).toHaveText(
          "기록을 저장했습니다.",
        );
        await expect(two.getByTestId("recording")).toHaveText("Result saved.");
        expect(first.lastView()?.result?.reason).toBe("timeout");
        for (const event of first.events) {
          expect(JSON.stringify(event)).not.toMatch(
            /"seed"|"truth"|"account_id"|"csrf"|"credential"/,
          );
        }
        for (const command of first.commands)
          expect(Object.keys(command).sort()).toEqual(
            [
              "v",
              "match_id",
              "command_id",
              "client_seq",
              "session_epoch",
              "known_revision",
              "action",
            ].sort(),
          );
        expect(
          requests.filter((url) =>
            /doubleclick|googlesyndication|adsbygoogle/.test(url),
          ),
        ).toHaveLength(0);
        const stored = await one.evaluate(() =>
          JSON.stringify({ ...localStorage, ...sessionStorage }),
        );
        expect(stored).not.toContain(first.snapshot()!.match_id);
        expect(stored).not.toMatch(/csrf|credential|seed|online_input/);
        expect(
          await one.evaluate(
            () => document.documentElement.scrollWidth <= innerWidth,
          ),
        ).toBe(true);
        await mkdir(".tmp/online-layout", { recursive: true });
        await one.screenshot({
          path: `.tmp/online-layout/result-${width}.png`,
          fullPage: true,
        });
      } finally {
        await a.close();
        await b.close();
      }
    });
  for (const difficulty of ["Easy", "Normal", "Hard"])
    test(`actual 10-second ${difficulty} bot assignment and authoritative progress`, async ({
      page,
      context,
    }) => {
      await approve(context);
      await login(page);
      const frames = stream(page);
      await page.goto("/en/queue");
      await page.getByRole("button", { name: difficulty, exact: true }).click();
      await page.getByRole("button", { name: "Find an opponent" }).click();
      await expect(
        page.getByRole("button", { name: "Cancel waiting" }),
      ).toBeVisible();
      await page.reload();
      await expect(
        page.getByRole("button", { name: "Cancel waiting" }),
      ).toBeVisible();
      await expect(
        page.getByRole("button", { name: difficulty, exact: true }),
      ).toHaveAttribute("aria-pressed", "true");
      await advance(page, 9);
      await expect(
        page.getByRole("button", { name: "Cancel waiting" }),
      ).toBeVisible();
      expect(frames.snapshot()).toBeUndefined();
      await advance(page, 1);
      await expect(page.getByRole("gridcell")).toHaveCount(9);
      await expect(page.getByText(`BOT · ${difficulty}`)).toBeVisible();
      await advance(page, 3);
      await expect.poll(() => frames.lastView()?.phase).toBe("playing");
      await advance(page, 2);
      await expect
        .poll(() => frames.lastView()?.opponent.opened_safe ?? 0)
        .toBeGreaterThan(4);
      await advance(page, 240);
      await expect(page.getByTestId("recording")).toHaveText("Result saved.");
    });
  test("actual queue cancel stays idle, locale changes preserve waiting, and revoked session removes the online view", async ({
    page,
    context,
  }) => {
    await approve(context);
    await login(page);
    const frames = stream(page);
    await page.goto("/en/queue");
    await page.getByRole("button", { name: "Find an opponent" }).click();
    await expect(
      page.getByRole("button", { name: "Cancel waiting" }),
    ).toBeVisible();
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
      await page.locator(".language-control select").selectOption(locale);
      await expect(page.locator("html")).toHaveAttribute("lang", locale);
      await expect(page.locator(".queue-clock")).toBeVisible();
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
    }
    await mkdir(".tmp/online-layout", { recursive: true });
    await page.screenshot({
      path: ".tmp/online-layout/waiting.png",
      fullPage: true,
    });
    await page.getByRole("button", { name: "Cancel waiting" }).click();
    await expect(
      page.getByRole("button", { name: "Find an opponent" }),
    ).toBeEnabled();
    await advance(page, 10);
    expect(frames.snapshot()).toBeUndefined();
    await page.getByRole("button", { name: "Find an opponent" }).click();
    await expect(
      page.getByRole("button", { name: "Cancel waiting" }),
    ).toBeVisible();
    await advance(page, 10);
    await expect(page.getByRole("gridcell")).toHaveCount(9);
    await advance(page, 3);
    await expect.poll(() => frames.lastView()?.phase).toBe("playing");
    const bootstrap = await page.request
      .get("/api/v1/auth/bootstrap")
      .then((r) => r.json());
    const logout = await page.request.post("/api/v1/auth/logout", {
      data: {},
      headers: { origin, "x-liar-csrf": bootstrap.csrf },
    });
    expect(logout.status()).toBe(204);
    await expect(page.locator("[data-cell]")).toHaveCount(0);
    await expect(
      page.getByRole("link", { name: "Practice without signing in" }),
    ).toBeVisible();
  });
}
