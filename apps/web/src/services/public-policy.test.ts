import { expect, test } from "vitest";
import { decodePublicPolicy, decodePublicSiteConfig } from "./public-policy";
import { LOCALES } from "./locale";
const policy = {
  revision: "test",
  effectiveDate: "2026-11-14",
  operator: "<Operator>",
  contact: "contact@example.test",
  privacy: [{ heading: "Privacy", body: "<script>escaped</script>" }],
  terms: [{ heading: "Terms", body: "Test text" }],
};
test("public configuration requires all languages, bounded plain text and exact HTTPS CSP origins", () => {
  const config = {
    policies: Object.fromEntries(LOCALES.map((locale) => [locale, policy])),
    google: {
      frame: ["https://example.test"],
      connect: ["https://example.test"],
      image: ["https://example.test"],
    },
  };
  expect(decodePublicSiteConfig(config).policies.ko.operator).toBe(
    "<Operator>",
  );
  for (const invalid of [
    "*",
    "https://*.example.test",
    "https://example.test/path",
    "http://example.test",
    "https://127.0.0.1",
    "https://db.localhost",
    "https://example.test'; script-src *",
  ])
    expect(() =>
      decodePublicSiteConfig({
        ...config,
        google: { ...config.google, frame: [invalid] },
      }),
    ).toThrow();
  expect(() =>
    decodePublicSiteConfig({ ...config, policies: { en: policy } }),
  ).toThrow();
  expect(() => decodePublicPolicy({ ...policy, extra: true })).toThrow();
  expect(() =>
    decodePublicPolicy({ ...policy, effectiveDate: "2026-02-30" }),
  ).toThrow();
});
