import { render } from "preact";
import { App } from "./App";
import { createI18n } from "./services/i18n";
import { createAutoAdsBrowser } from "./services/ads/browser";
import { createAdSenseScript } from "./services/ads/adsense";
import { createPreferences } from "./services/preferences";
import { createLearning } from "./services/learning";
import { createDailyRecords } from "./services/daily-records";
import { createSharePort } from "./services/daily-share";
import { createActivity } from "./services/activity";
import { createOffline } from "./services/offline";
import { createBrowserConsent } from "./services/consent-browser";
import { createOptionalEvents } from "./services/privacy/collector";
import { bindOptionalLanding } from "./services/privacy/browser";
import type { ConsentControlPort } from "./services/privacy/types";
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
const consent = createBrowserConsent(window, account.auth);
const learning = createLearning(storage);
const autoAds = createAutoAdsBrowser(window, {
  script: createAdSenseScript(document, import.meta.env.VITE_ADSENSE_CLIENT_ID),
  consent,
  auth: account.auth,
  activity,
  learning,
  preferences,
});
const navigation = autoAds.navigation;
let controlHttp: Promise<ConsentControlPort> | undefined;
function getConsentHttp(): Promise<ConsentControlPort> {
  return (controlHttp ??= import("./services/privacy/http")
    .then((module) =>
      module.createConsentHttp(account.auth, window.fetch.bind(window), () =>
        optionalEvents.cancel(),
      ),
    )
    .catch((error) => {
      controlHttp = undefined;
      throw error;
    }));
}
const consentControl: ConsentControlPort = {
  read: async (...args) => (await getConsentHttp()).read(...args),
  refresh: async (...args) => {
    optionalEvents.cancel();
    return (await getConsentHttp()).refresh(...args);
  },
  withdraw: async (...args) => {
    optionalEvents.cancel();
    return (await getConsentHttp()).withdraw(...args);
  },
};
const optionalEvents = createOptionalEvents({
  auth: account.auth,
  consent,
  control: consentControl,
  transport: {
    send: async (...args) =>
      (await import("./services/privacy/http"))
        .createEventHttp(account.auth, window.fetch.bind(window))
        .send(...args),
  },
  available: () => navigator.onLine && document.visibilityState === "visible",
  clock: () => ({ wall: Date.now(), monotonic: performance.now() }),
  id: () => crypto.randomUUID(),
});
const optionalLanding = bindOptionalLanding(
  window,
  navigation,
  account.auth,
  consent,
  optionalEvents,
);
import.meta.hot?.dispose(() => {
  autoAds.dispose();
  optionalLanding.dispose();
  optionalEvents.dispose();
  consent.dispose();
});
render(
  <App
    services={{
      consent,
      consentControl,
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
      me: {
        overview: async (...args) =>
          (await import("./services/me/http"))
            .createMeHttp(account.auth, window.fetch.bind(window))
            .overview(...args),
        history: async (...args) =>
          (await import("./services/me/http"))
            .createMeHttp(account.auth, window.fetch.bind(window))
            .history(...args),
      },
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
      learning,
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
