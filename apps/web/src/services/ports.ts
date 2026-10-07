import type { PracticeCore, TrainingCore, DailyCore } from "@liar/core-bridge";
import type { DailyRecordsPort } from "./daily-records";
import type { SharePort } from "./daily-share";
import type { LearningPort } from "./learning";
import type { I18nPort } from "./i18n";
import type { NavigationPort } from "./navigation";
import type { PreferencesPort } from "./preferences";
import type { BoardRendererFactory } from "../board/renderer";
export interface AppServices {
  i18n: I18nPort;
  navigation: NavigationPort;
  preferences: PreferencesPort;
  practiceCore(): Promise<PracticeCore>;
  trainingCore(): Promise<TrainingCore>;
  learning: LearningPort;
  dailyCore(): Promise<DailyCore>;
  dailyRecords: DailyRecordsPort;
  share: SharePort;
  wallClock(): number;
  boardRenderer: BoardRendererFactory;
}
