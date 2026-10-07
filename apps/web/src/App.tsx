import { useEffect, useState } from "preact/hooks";
import { Ui } from "./ui/context";
import { AppShell } from "./ui/templates/AppShell";
import { HomePage } from "./pages/HomePage";
import { SettingsPage } from "./pages/SettingsPage";
import { RulesPage } from "./pages/RulesPage";
import { StatusPage } from "./pages/StatusPage";
import { chooseLocale } from "./services/locale";
import type { AppServices } from "./services/ports";
export function App({ services }: { services: AppServices }) {
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
  const page =
    route === "/" ? (
      <HomePage />
    ) : route === "/settings" ? (
      <SettingsPage />
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
