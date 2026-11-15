import { expect, test } from "vitest";
import { precacheDocumentURL } from "../../build/precache-documents";
import { LOCALES } from "./locale";
import { PUBLIC_DOCUMENTS } from "./seo/document";
test("precache HTML uses served clean document URLs and leaves assets untouched", () => {
  expect(precacheDocumentURL("index.html")).toBe("/");
  for (const locale of LOCALES)
    for (const route of PUBLIC_DOCUMENTS) {
      const physical =
        route === "/" ? `${locale}/index.html` : `${locale}${route}.html`;
      expect(precacheDocumentURL(physical)).toBe(`/${locale}${route}`);
    }
  for (const path of [
    "assets/test.js",
    "policies/en.json",
    "untrusted.html",
    "en/me.html",
  ])
    expect(precacheDocumentURL(path)).toBe(path);
});
