import { LOCALES, LOCALE_NAMES, isLocale } from "../../services/locale";
import { useUi } from "../context";
export function LanguageSelect() {
  const { locale, t, selectLocale } = useUi();
  return (
    <select
      aria-label={t("language")}
      value={locale}
      onChange={(event) => {
        const value = event.currentTarget.value;
        if (isLocale(value)) selectLocale(value);
      }}
    >
      {LOCALES.map((language) => (
        <option key={language} value={language} lang={language}>
          {LOCALE_NAMES[language]}
        </option>
      ))}
    </select>
  );
}
