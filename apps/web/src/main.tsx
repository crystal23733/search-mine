import { render } from "preact";
import { App } from "./App";
import { createI18n } from "./services/i18n";
import { createNavigation } from "./services/navigation";
import { createPreferences } from "./services/preferences";
import { createLearning } from "./services/learning";
import { createDailyRecords } from "./services/daily-records";
import { createSharePort } from "./services/daily-share";
import { createActivity } from "./services/activity";
import { createOffline } from "./services/offline";
import { createBrowserConsent } from "./services/consent-browser";
import { createBrowserAuth } from "./services/auth/browser";
import { createAuthHttp } from "./services/auth/http";
import { createPendingSubmissions } from "./services/pending-submissions";
import { chooseLocale, localizedPath } from "./services/locale";
import "@liar/design-tokens/tokens.css";
import "./styles.css";
import type { DailyApi } from "./services/daily/types";
const root = document.getElementById("app");
if (!root) throw new Error("Application root is missing");
let storage: Storage | undefined;
try {
  storage = window.localStorage;
} catch {
  /* Essential preferences use memory when storage is unavailable. */
}
const preferences = createPreferences(storage);
let database: IDBFactory | undefined;
try {
  database = window.indexedDB;
} catch {
  /* Local records remain available in memory. */
}
const locale = chooseLocale(
  location.pathname,
  preferences.read().locale,
  navigator.languages,
);
if (!location.pathname.startsWith(`/${locale}/`))
  history.replaceState(
    null,
    "",
    localizedPath(location.pathname, locale) + location.search + location.hash,
  );
const navigation = createNavigation(window);
const i18n = await createI18n(locale);
const activity = createActivity();
const account = createBrowserAuth(
  window,
  createAuthHttp(window.fetch.bind(window)),
);
const records = createDailyRecords(database);
let dailyApi: DailyApi | undefined;
async function getDailyApi(): Promise<DailyApi> {
  const { createDailyHttp } = await import("./services/daily/http");
  return (dailyApi ??= createDailyHttp(
    account.auth,
    window.fetch.bind(window),
  ));
}
const pendingSubmissions = createPendingSubmissions(records, account.auth, {
  submit: async (record, accountId, signal) => {
    return (await getDailyApi()).submitPending(
      { accountId, revision: account.auth.revision() },
      record,
      signal,
    );
  },
});
const offline = createOffline(activity, {
  window,
  enabled: import.meta.env.PROD,
  storage,
});
render(
  <App
    services={{
      consent: createBrowserConsent(window, account.auth),
      dailyChallenge: {
        start: async (...args) => (await getDailyApi()).start(...args),
        heartbeat: async (...args) => (await getDailyApi()).heartbeat(...args),
        definition: async (...args) =>
          (await getDailyApi()).definition(...args),
        own: async (...args) => (await getDailyApi()).own(...args),
        submitTimed: async (...args) =>
          (await getDailyApi()).submitTimed(...args),
      },
      dailyRankings: async (...args) => (await getDailyApi()).rankings(...args),
      auth: account.auth,
      online: {
        latestResult: async (...args) =>
          (
            await import("./services/online/result-http")
          ).createLatestResultHttp(
            account.auth,
            window.fetch.bind(window),
          )(...args),
        result: async (...args) =>
          (await import("./services/online/result-http")).createResultHttp(
            account.auth,
            window.fetch.bind(window),
          )(...args),
        lobby: async (...args) =>
          (await import("./services/online/http")).createLobbyHttp(
            account.auth,
            window.fetch.bind(window),
          )(...args),
        connect: async (...args) =>
          (await import("./services/online/socket")).createOnlineSocket(
            account.auth,
            window.location.origin,
            (url) => new WebSocket(url),
          )(...args),
      },
      authEffects: account.effects,
      pendingSubmissions,
      activity,
      offline,
      i18n,
      navigation,
      preferences,
      learning: createLearning(storage),
      dailyRecords: records,
      share: createSharePort(),
      wallClock: () => Date.now(),
      dailyCore: async () =>
        (await import("./services/core")).createDailyCore(),
      trainingCore: async () =>
        (await import("./services/core")).createTrainingCore(),
      practiceCore: async () =>
        (await import("./services/core")).createPracticeCore(),
      boardRenderer: async (host) =>
        (await import("./board/pixi")).createPixiBoard(host),
    }}
  />,
  root,
);
