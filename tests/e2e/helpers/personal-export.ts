import { test, expect, type BrowserContext, type Page } from "@playwright/test";
import { readFile } from "node:fs/promises";
import type { PersonalDataExport } from "../../../packages/protocol/src/index";
import { advance, stream } from "./online-quick";
type Approve = (context: BrowserContext) => Promise<void>;
type Login = (page: Page) => Promise<void>;
const origin = "https://localhost:8443";
export function registerPersonalExportTests(approve: Approve, login: Login) {
  for (const width of [1440, 390])
    test(`complete own online export downloads in eight languages at ${width}px`, async ({
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
        await one.goto("/en/queue");
        await one.getByRole("button", { name: "Find an opponent" }).click();
        await expect(
          one.getByRole("button", { name: "Cancel waiting" }),
        ).toBeVisible();
        await two.goto("/en/queue");
        await two.getByRole("button", { name: "Find an opponent" }).click();
        await expect(one.getByRole("gridcell")).toHaveCount(9);
        await advance(one, 3);
        await expect.poll(() => observed.lastView()?.phase).toBe("playing");
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
          await one.goto(`/${locale}/settings`);
          const button = one.getByRole("button", {
            name: messages["auth.export"],
            exact: true,
          });
          await expect(button).toBeEnabled();
          const waiting = one.waitForEvent("download");
          await button.click();
          const artifact = await waiting;
          expect(artifact.suggestedFilename()).toBe("liar-account.json");
          const reader = await artifact.createReadStream();
          const chunks: Buffer[] = [];
          for await (const chunk of reader!) chunks.push(Buffer.from(chunk));
          const value = JSON.parse(
            Buffer.concat(chunks).toString("utf8"),
          ) as PersonalDataExport;
          expect(Object.keys(value).sort()).toEqual([
            "account",
            "games",
            "replays",
            "v",
          ]);
          expect(value.v).toBe(1);
          expect(value.games).toHaveLength(1);
          expect(value.replays).toEqual([]);
          const record = value.games[0];
          expect(record.kind).toBe("online");
          if (record.kind !== "online")
            throw Error("missing own online result");
          expect(record.result.match_id).toBe(matchId);
          expect(record.result.own?.opened_safe).toBe(ownOpened);
          expect(JSON.stringify(value)).not.toMatch(
            /secret|seed|board|subject|digest|credential|csrf|session_revision|token|password|email/,
          );
        }
      } finally {
        await a.close();
        await b.close();
      }
    });
}
