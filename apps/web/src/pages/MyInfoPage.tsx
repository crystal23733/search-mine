import { useEffect } from "preact/hooks";
import { useUi } from "../ui/context";
import { useSnapshot } from "../ui/useSnapshot";
import { NavLink } from "../ui/molecules/NavLink";
import { Button } from "../ui/atoms/Button";
import { Card } from "../ui/atoms/Card";
import { AuthFeedback } from "../ui/organisms/AuthFeedback";
import { ConsentSettings } from "../ui/organisms/ConsentSettings";
import { MeContent, type MeSection } from "../ui/organisms/MeContent";
export function MyInfoPage({ section = "overview" }: { section?: MeSection }) {
  const { t, services } = useUi();
  const auth = useSnapshot(services.auth);
  const offline = useSnapshot(services.offline);
  useEffect(() => {
    const previous = document.querySelector<HTMLMetaElement>(
      'meta[name="robots"]',
    );
    const original = previous?.getAttribute("content");
    const meta = previous ?? document.createElement("meta");
    meta.name = "robots";
    meta.content = "noindex, noarchive";
    if (!previous) document.head.append(meta);
    return () => {
      if (!previous) meta.remove();
      else if (original === null) meta.removeAttribute("content");
      else meta.content = original ?? "";
    };
  }, []);
  const ready =
    auth.status === "ready" &&
    auth.account !== null &&
    (auth.working || services.auth.connected());
  const title =
    section === "overview"
      ? "me.title"
      : section === "history"
        ? "me.history"
        : section === "account"
          ? "me.account"
          : "me.privacy";
  return (
    <div class="reading-page me-page">
      <h1>{t(title)}</h1>
      <nav class="me-nav" aria-label={t("me.title")}>
        <NavLink path="/me">{t("me.title")}</NavLink>
        <NavLink path="/me/history">{t("me.history")}</NavLink>
        <NavLink path="/me/account">{t("me.account")}</NavLink>
        <NavLink path="/me/privacy">{t("me.privacy")}</NavLink>
        <NavLink path="/settings">{t("settings")}</NavLink>
      </nav>
      {ready ? (
        <MeContent key={`${auth.account!.id}-${section}`} section={section} />
      ) : (
        <Card>
          {auth.status !== "loading" && <AuthFeedback />}
          <p role={auth.status === "unavailable" ? "alert" : "status"}>
            {t(
              auth.status === "loading" || auth.working
                ? "auth.loading"
                : !offline.online
                  ? "me.offline"
                  : auth.status === "unavailable"
                    ? "me.error"
                    : "me.signIn",
            )}
          </p>
          <div class="match-controls">
            {auth.status === "unavailable" && (
              <Button
                onClick={() => {
                  void services.auth.refresh();
                }}
              >
                {t("me.retry")}
              </Button>
            )}
            <NavLink
              path="/login"
              query={{ return_path: "me" }}
              class="button button--primary"
            >
              {t("me.login")}
            </NavLink>
            <NavLink path="/settings">{t("settings")}</NavLink>
          </div>
        </Card>
      )}
      {section === "privacy" && <ConsentSettings />}
    </div>
  );
}
