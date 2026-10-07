import { createContext } from "preact";
import { useContext } from "preact/hooks";
import type { Locale } from "../services/locale";
import type { MessageKey } from "../services/i18n";
import type { Preferences } from "../services/preferences";
import type { AppServices } from "../services/ports";
export interface UiContext {
  locale: Locale;
  t(key: MessageKey, data?: Record<string, string | number>): string;
  services: AppServices;
  preferences: Preferences;
  selectLocale(locale: Locale): void;
}
export const Ui = createContext<UiContext | undefined>(undefined);
export function useUi() {
  const ui = useContext(Ui);
  if (!ui) throw new Error("UI provider is required");
  return ui;
}
