// Browser test composition only. This module is not an application build entry point.
import { createAutoAdsBrowser } from "../src/services/ads/browser";
import { createAdSenseScript } from "../src/services/ads/adsense";
import { createActivity } from "../src/services/activity";
import { createLearning } from "../src/services/learning";
import { createPreferences } from "../src/services/preferences";
import type { AuthState } from "../src/services/auth/types";
export function install(allowed = true) {
  const listeners = new Set<() => void>();
  const subscribe = (listener: () => void) => {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  };
  const state: AuthState = {
    status: "ready",
    account: null,
    providers: [],
    identities: [],
    working: false,
    error: null,
    notice: null,
    manualAppleDisconnect: false,
    appleRevokePending: false,
  };
  const app = createAutoAdsBrowser(window, {
    script: createAdSenseScript(document, "ca-pub-1234567890123456"),
    consent: {
      read: () => ({
        status: allowed ? "valid" : "denied",
        advertising: allowed,
        analytics: false,
        working: false,
        canReopen: true,
      }),
      subscribe,
    },
    auth: {
      read: () => state,
      account: () => null,
      revision: () => 1,
      subscribe,
    },
    activity: createActivity(),
    learning: createLearning(localStorage),
    preferences: createPreferences(localStorage),
  });
  return {
    go: (path: string) => app.navigation.go(path),
    consent(value: boolean) {
      allowed = value;
      for (const listener of [...listeners]) listener();
    },
    dispose: app.dispose,
  };
}
