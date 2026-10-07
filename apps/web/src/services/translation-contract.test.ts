import { describe, expect, it } from "vitest";
import { readdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { createI18n } from "./i18n";
import { LOCALES } from "./locale";
import en from "../locales/en/common.json";
import tokens from "../../../../packages/design-tokens/tokens.json";
const placeholders = (value: string) =>
  [...value.matchAll(/{{\s*([^}\s]+)\s*}}/g)].map((match) => match[1]).sort();
describe("translation and semantic token contracts", () => {
  it("validates exact English keys and interpolation variables in all eight locales", async () => {
    const root = resolve(process.cwd(), "src/locales");
    expect(readdirSync(root).sort()).toEqual([...LOCALES].sort());
    const adapter = await createI18n("en");
    for (const locale of LOCALES) {
      const source: Record<string, string> = JSON.parse(
        readFileSync(resolve(root, locale, "common.json"), "utf8"),
      );
      expect(Object.keys(source).sort()).toEqual(Object.keys(en).sort());
      for (const key of Object.keys(en) as Array<keyof typeof en>) {
        expect(source[key].length).toBeGreaterThan(0);
        expect(placeholders(source[key])).toEqual(placeholders(en[key]));
      }
      await adapter.load(locale);
      expect(
        adapter.t(locale, "rules.win", { safe: 216, minutes: "4 min" }),
      ).not.toContain("{{");
      expect(adapter.t(locale, "home.description")).toBe(
        source["home.description"],
      );
    }
  });
  it("meets normal-text contrast for actual foreground/background semantic pairs", () => {
    const luminance = (hex: string) => {
      const rgb = [1, 3, 5]
        .map((offset) => parseInt(hex.slice(offset, offset + 2), 16) / 255)
        .map((value) =>
          value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4,
        );
      return rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
    };
    const ratio = (a: string, b: string) => {
      const values = [luminance(a), luminance(b)].sort((x, y) => y - x);
      return (values[0] + 0.05) / (values[1] + 0.05);
    };
    for (const mode of [
      tokens.colors,
      { ...tokens.colors, ...tokens["high-contrast"] },
    ]) {
      for (const surface of [mode.background, mode.surface, mode.raised])
        for (const text of [
          mode.text,
          mode.muted,
          mode.danger,
          mode.success,
          mode.number1,
        ])
          expect(ratio(text, surface)).toBeGreaterThanOrEqual(4.5);
      expect(ratio(mode.accent, mode["on-accent"])).toBeGreaterThanOrEqual(4.5);
    }
  });
});
