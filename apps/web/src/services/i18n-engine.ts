import i18next from "i18next";
import en from "../locales/en/common.json" with { type: "json" };
import type { Locale } from "./locale.ts";
export type MessageKey = keyof typeof en;
export interface I18nPort {
  load(locale: Locale): Promise<void>;
  t(
    locale: Locale,
    key: MessageKey,
    data?: Record<string, string | number>,
  ): string;
}

export async function createTranslation(
  english: Record<MessageKey, string>,
  loadResources: (
    locale: Exclude<Locale, "en">,
  ) => Promise<Record<MessageKey, string>>,
): Promise<I18nPort> {
  const instance = i18next.createInstance();
  await instance.init({
    resources: { en: { common: english } },
    lng: "en",
    fallbackLng: "en",
    defaultNS: "common",
    keySeparator: false,
    load: "currentOnly",
    interpolation: { escapeValue: false },
  });
  return {
    load: async (language) => {
      if (language !== "en" && !instance.hasResourceBundle(language, "common"))
        instance.addResourceBundle(
          language,
          "common",
          await loadResources(language),
        );
    },
    t: (language, key, data) =>
      String(
        instance.t(key, { lng: language, defaultValue: english[key], ...data }),
      ),
  };
}
