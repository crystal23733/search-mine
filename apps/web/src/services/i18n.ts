import en from "../locales/en/common.json";
import type { Locale } from "./locale.ts";
import { createTranslation, type I18nPort } from "./i18n-engine";
export type { I18nPort, MessageKey } from "./i18n-engine";
const loaders = {
  ko: () => import("../locales/ko/common.json"),
  ja: () => import("../locales/ja/common.json"),
  "zh-CN": () => import("../locales/zh-CN/common.json"),
  es: () => import("../locales/es/common.json"),
  "pt-BR": () => import("../locales/pt-BR/common.json"),
  de: () => import("../locales/de/common.json"),
  fr: () => import("../locales/fr/common.json"),
};
export async function createI18n(locale: Locale): Promise<I18nPort> {
  const port = await createTranslation(
    en,
    async (language) => (await loaders[language]()).default,
  );
  try {
    await port.load(locale);
  } catch {
    /* English remains available when a locale chunk fails. */
  }
  return port;
}
