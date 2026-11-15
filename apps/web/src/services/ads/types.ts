import type { ActivityPort } from "../activity";
import type { AuthPort } from "../auth/types";
import type { ConsentPort } from "../consent";
import type { LearningPort } from "../learning";
import type { PreferencesPort } from "../preferences";

export interface AutoAdsScriptPort {
  configured(): boolean;
  load(signal: AbortSignal): Promise<void>;
}
export interface DocumentNavigation {
  navigate(url: string): void;
  reload(): void;
}
export interface AutoAdsDependencies {
  script: AutoAdsScriptPort;
  messaging?: {
    eligible(url: URL): boolean;
    prepared(): boolean;
    documentReady?(): boolean;
    prepare(signal: AbortSignal): Promise<void>;
  };
  consent: Pick<ConsentPort, "read" | "subscribe">;
  auth: Pick<AuthPort, "read" | "account" | "revision" | "subscribe">;
  activity: Pick<ActivityPort, "read" | "subscribe">;
  learning: Pick<LearningPort, "read" | "persistent">;
  preferences: Pick<PreferencesPort, "persistent" | "subscribe">;
  documents?: DocumentNavigation;
}
