import { render } from "preact";
import { App } from "./App";
import { createI18n } from "./services/i18n";
import { createNavigation } from "./services/navigation";
import { createPreferences } from "./services/preferences";
import { createLearning } from "./services/learning";
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
render(
  <App
    services={{
      i18n,
      navigation,
      preferences,
      learning: createLearning(storage),
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
