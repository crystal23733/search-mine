import { LOCALES } from "../src/services/locale.ts";
import { PUBLIC_DOCUMENTS } from "../src/services/seo/document.ts";
const servedDocuments = new Map<string, string>([
  ["index.html", "/"],
  ...LOCALES.flatMap((locale) =>
    PUBLIC_DOCUMENTS.map(
      (route) =>
        [
          route === "/" ? `${locale}/index.html` : `${locale}${route}.html`,
          `/${locale}${route}`,
        ] as [string, string],
    ),
  ),
]);
export const precacheDocumentURL = (url: string) =>
  servedDocuments.get(url) ?? url;
