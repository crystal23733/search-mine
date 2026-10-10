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
import { createBrowserAuth } from "./services/auth/browser";
import { createAuthHttp } from "./services/auth/http";
import { createPendingSubmissions } from "./services/pending-submissions";
import { chooseLocale, localizedPath } from "./services/locale";
import "@liar/design-tokens/tokens.css";
import "./styles.css";
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
const pendingSubmissions = createPendingSubmissions(records, account.auth, {
  submit: async () => {
    throw Error("official_submission_unavailable");
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
      auth: account.auth,
      online: {
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
