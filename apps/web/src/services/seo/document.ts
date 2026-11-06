import { LOCALES, type Locale } from "../locale.ts";
import { documentRoute, documentStatus } from "../document-route.ts";
import type { I18nPort } from "../i18n.ts";
export const PUBLIC_DOCUMENTS = ["/", "/rules", "/storage", "/learn"] as const;
export type PublicDocument = (typeof PUBLIC_DOCUMENTS)[number];
export function publicOrigin(value: string | undefined): string | undefined {
  if (!value) return undefined;
  const url = new URL(value);
  if (
    url.origin !== value.replace(/\/$/, "") ||
    url.protocol !== "https:" ||
    url.username ||
    url.password ||
    url.port ||
    url.pathname !== "/" ||
    url.search ||
    url.hash ||
    !url.hostname.includes(".") ||
    url.hostname.endsWith(".") ||
    url.hostname.endsWith(".localhost") ||
    url.hostname.includes(":") ||
    /^[\d.]+$/.test(url.hostname)
  )
    throw new Error(
      "VITE_PUBLIC_SITE_ORIGIN must be a public HTTPS origin without credentials, port or path",
    );
  return url.origin;
}
export function publicPath(path: string): PublicDocument | undefined {
  return PUBLIC_DOCUMENTS.find((route) => route === path);
}
export function localizedDocument(
  locale: Locale,
  route: PublicDocument,
): string {
  return `/${locale}${route}`;
}
export function documentMetadata(
  url: URL,
  locale: Locale,
  i18n: I18nPort,
  origin?: string,
) {
  origin = publicOrigin(origin);
  const route = documentRoute(url.pathname);
  const t = (key: Parameters<I18nPort["t"]>[1]) => i18n.t(locale, key);
  const status = documentStatus(url.pathname);
  const page = publicPath(route);
  const readable =
    page !== undefined &&
    !url.search &&
    (!url.hash || url.hash === "#main") &&
    url.pathname.split("/")[1] === locale;
  const heading =
    status !== 200
      ? `${status} · ${t(status === 403 ? "error.forbiddenTitle" : "error.notFoundTitle")}`
      : page === "/learn"
        ? t("learn.title")
        : page === "/rules"
          ? t("rules.title")
          : page === "/storage"
            ? t("storage.pageTitle")
            : page === "/"
              ? `${t("home.title1")} ${t("home.title2")}`
              : "Liar Sweeper";
  const description =
    page === "/learn"
      ? t("learn.description")
      : page === "/rules"
        ? t("basics.goal")
        : page === "/storage"
          ? t("storage.intro")
          : page === "/"
            ? t("home.description")
            : undefined;
  const canonical =
    readable && origin
      ? `${origin}${localizedDocument(locale, page)}`
      : undefined;
  return {
    locale,
    title: heading === "Liar Sweeper" ? heading : `${heading} | Liar Sweeper`,
    description: readable ? description : undefined,
    noindex: !readable,
    canonical,
    alternates: canonical
      ? [
          ...LOCALES.map((language) => ({
            language,
            href: `${origin}${localizedDocument(language, page!)}`,
          })),
          {
            language: "x-default",
            href: `${origin}${localizedDocument("en", page!)}`,
          },
        ]
      : [],
  };
}
export type DocumentMetadata = ReturnType<typeof documentMetadata>;
