import i18next from "i18next";
import en from "../locales/en/common.json";
import type { Locale } from "./locale";
export type MessageKey = keyof typeof en;
export interface I18nPort {
  load(locale: Locale): Promise<void>;
  t(
    locale: Locale,
    key: MessageKey,
    data?: Record<string, string | number>,
  ): string;
}
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
  const instance = i18next.createInstance();
  await instance.init({
    resources: { en: { common: en } },
    lng: "en",
    fallbackLng: "en",
    defaultNS: "common",
    keySeparator: false,
    load: "currentOnly",
    interpolation: { escapeValue: false },
  });
  const port: I18nPort = {
    load: async (language) => {
      if (language !== "en" && !instance.hasResourceBundle(language, "common"))
        instance.addResourceBundle(
          language,
          "common",
          (await loaders[language]()).default,
        );
    },
    t: (language, key, data) =>
      String(
        instance.t(key, { lng: language, defaultValue: en[key], ...data }),
      ),
  };
  try {
    await port.load(locale);
  } catch {
    /* English remains available when a locale chunk fails. */
  }
  return port;
}
