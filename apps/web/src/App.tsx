import { useEffect, useState } from "preact/hooks";
import { Ui } from "./ui/context";
import { AppShell } from "./ui/templates/AppShell";
import { HomePage } from "./pages/HomePage";
import { LatestResultPage } from "./pages/LatestResultPage";
import { SettingsPage } from "./pages/SettingsPage";
import { RulesPage } from "./pages/RulesPage";
import { StatusPage } from "./pages/StatusPage";
import { LoginPage } from "./pages/LoginPage";
import { OnboardingPage } from "./pages/OnboardingPage";
import { GameRoute } from "./pages/GameRoute";
import { chooseLocale } from "./services/locale";
import type { AppServices } from "./services/ports";
export function App({ services }: { services: AppServices }) {
  useEffect(() => {
    void services.auth.refresh();
  }, [services]);
  const [url, setUrl] = useState(services.navigation.current());
  const [preferences, setPreferences] = useState(services.preferences.read());
  const requestedLocale = chooseLocale(url.pathname, preferences.locale, []);
  const [locale, setLocale] = useState(requestedLocale);
  useEffect(
    () =>
      services.navigation.subscribe(() =>
        setUrl(services.navigation.current()),
      ),
    [services],
  );
  useEffect(
    () =>
      services.preferences.subscribe(() =>
        setPreferences(services.preferences.read()),
      ),
    [services],
  );
  useEffect(() => {
    let current = true;
    void services.i18n
      .load(requestedLocale)
      .then(() => {
        if (current) setLocale(requestedLocale);
      })
      .catch(() => {
        if (current) setLocale("en");
      });
    return () => {
      current = false;
    };
  }, [services, requestedLocale]);
  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);
  useEffect(() => {
    document.documentElement.dataset.contrast = preferences.contrast;
    document.documentElement.dataset.motion = preferences.motion;
    document.documentElement.style.setProperty(
      "--board-scale",
      String(preferences.zoom),
    );
  }, [preferences]);
  const route =
    url.pathname.replace(/^\/(en|ko|ja|zh-CN|es|pt-BR|de|fr)(?=\/|$)/, "") ||
    "/";
  useEffect(() => {
    if (
      services.learning.read() === "new" &&
      ["/", "/practice", "/friends", "/daily", "/queue"].includes(route)
    )
      services.navigation.go("/tutorial", { return: route });
  }, [services, route]);
  const page =
    route === "/practice" ||
    route === "/tutorial" ||
    route === "/daily" ||
    route === "/queue" ||
    route === "/friends" ? (
      <GameRoute
        route={
          route === "/friends"
            ? "friends"
            : route === "/queue"
              ? "queue"
              : route === "/practice"
                ? "practice"
                : route === "/daily"
                  ? "daily"
                  : "tutorial"
        }
      />
    ) : route === "/" ? (
      <HomePage />
    ) : route === "/results" ? (
      <LatestResultPage />
    ) : route === "/settings" ? (
      <SettingsPage />
    ) : route === "/login" ? (
      <LoginPage />
    ) : route === "/onboarding" ? (
      <OnboardingPage />
    ) : route === "/rules" ? (
      <RulesPage />
    ) : (
      <StatusPage online={["/queue", "/friends", "/login"].includes(route)} />
    );
  return (
    <Ui.Provider
      value={{
        locale,
        preferences,
        services,
        t: (key, data) => services.i18n.t(locale, key, data),
        selectLocale: (language) => {
          services.preferences.update({ locale: language });
          services.navigation.locale(language);
        },
      }}
    >
      <AppShell>{page}</AppShell>
    </Ui.Provider>
  );
}
