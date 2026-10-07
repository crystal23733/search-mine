import type { PracticeCore } from "@liar/core-bridge";
import type { I18nPort } from "./i18n";
import type { NavigationPort } from "./navigation";
import type { PreferencesPort } from "./preferences";
import type { BoardRendererFactory } from "../board/renderer";
export interface AppServices {
  i18n: I18nPort;
  navigation: NavigationPort;
  preferences: PreferencesPort;
  practiceCore(): Promise<PracticeCore>;
  boardRenderer: BoardRendererFactory;
}
