import { useUi } from "../ui/context";
import { Card } from "../ui/atoms/Card";
import { NavLink } from "../ui/molecules/NavLink";
import { ProviderButtons } from "../ui/molecules/ProviderButton";
import { AuthFeedback } from "../ui/organisms/AuthFeedback";
export function LoginPage() {
  const { t, services } = useUi();
  const failed =
    new URLSearchParams(services.navigation.current().search).get("error") ===
    "auth_failed";
  return (
    <div class="auth-page reading-page">
      <div class="auth-step">
        <NavLink path="/">{t("home")}</NavLink>
        <span class="muted">{t("auth.step", { step: 1 })}</span>
      </div>
      <h1>{t("auth.title")}</h1>
      <p class="auth-intro">{t("auth.intro")}</p>
      {failed && <p role="alert">{t("auth.failed")}</p>}
      <AuthFeedback />
      <ProviderButtons />
      <Card class="privacy-card">
        <p>{t("auth.privacy")}</p>
      </Card>
      <NavLink path="/practice" class="auth-local">
        {t("auth.local")}
      </NavLink>
    </div>
  );
}
