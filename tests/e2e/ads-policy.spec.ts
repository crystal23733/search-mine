import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";
import type { AdPlan } from "@liar/protocol";

const cases = JSON.parse(
  readFileSync(new URL("../fixtures/native-ads.json", import.meta.url), "utf8"),
) as Array<{
  name: string;
  input: string;
  expected: { ok: AdPlan } | { error: string };
}>;

test("advertisement planning and closed decoding match native Rust in WASM", async ({
  page,
}) => {
  await page.addInitScript(() =>
    localStorage.setItem("liar.tutorial.v1", '"skipped"'),
  );
  await page.goto("/ko/");
  const actual = await page.evaluate(
    async (inputs) => {
      const uri = "/core/liar_wasm.js";
      const wasm = await import(/* @vite-ignore */ uri);
      await wasm.default();
      return inputs.map((input) => {
        try {
          return { ok: JSON.parse(wasm.planAdRequest(input)) };
        } catch (error) {
          return {
            error: error instanceof Error ? error.message : "unavailable",
          };
        }
      });
    },
    cases.map((scenario) => scenario.input),
  );
  for (let index = 0; index < cases.length; index++) {
    expect(actual[index], cases[index].name).toEqual(cases[index].expected);
  }
});
