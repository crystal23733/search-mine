export const LOCALES = [
  "en",
  "ko",
  "ja",
  "zh-CN",
  "es",
  "pt-BR",
  "de",
  "fr",
] as const;
export type Locale = (typeof LOCALES)[number];
export const LOCALE_NAMES: Record<Locale, string> = {
  en: "English",
  ko: "한국어",
  ja: "日本語",
  "zh-CN": "简体中文",
  es: "Español",
  "pt-BR": "Português (Brasil)",
  de: "Deutsch",
  fr: "Français",
};
export const isLocale = (value: unknown): value is Locale =>
  typeof value === "string" && (LOCALES as readonly string[]).includes(value);
export function normalizeLocale(language: string): Locale | undefined {
  const code = language.toLowerCase().replaceAll("_", "-");
  if (code.startsWith("zh")) {
    if (/(?:^|-)(hant|tw|hk|mo)(?:-|$)/.test(code)) return undefined;
    return "zh-CN";
  }
  if (code === "pt" || code.startsWith("pt-")) return "pt-BR";
  return LOCALES.find((locale) => locale === code.split("-")[0]);
}
export function chooseLocale(
  pathname: string,
  saved: Locale | undefined,
  languages: readonly string[],
): Locale {
  const prefix = pathname.split("/")[1];
  if (isLocale(prefix)) return prefix;
  if (pathname !== "/") return "en";
  if (saved) return saved;
  for (const language of languages) {
    const locale = normalizeLocale(language);
    if (locale) return locale;
  }
  return "en";
}
export function localizedPath(pathname: string, locale: Locale): string {
  const parts = pathname.split("/");
  if (isLocale(parts[1])) parts.splice(1, 1);
  return `/${locale}${parts.join("/") || "/"}`;
}
