import { useUi } from "../ui/context";
import { useSnapshot } from "../ui/useSnapshot";
import { NavLink } from "../ui/molecules/NavLink";
import { AuthFeedback } from "../ui/organisms/AuthFeedback";
import { NicknameForm } from "../ui/organisms/NicknameForm";
import { authReturn } from "../services/auth/types";
export function OnboardingPage() {
  const { t, services } = useUi(),
    state = useSnapshot(services.auth);
  const destination = authReturn(
    new URLSearchParams(services.navigation.current().search).get(
      "return_path",
    ),
  );
  const path = destination === "home" ? "/" : `/${destination}`;
  return (
    <div class="auth-page reading-page">
      <div class="auth-step">
        <NavLink path="/settings">{t("settings")}</NavLink>
        <span class="muted">{t("auth.step", { step: 2 })}</span>
      </div>
      <h1>{t("auth.nicknameTitle")}</h1>
      <p class="auth-intro">{t("auth.nicknamePrivacy")}</p>
      <AuthFeedback />
      {state.account ? (
        <NicknameForm
          key={state.account.id}
          account={state.account}
          onboarding
          onSaved={(_account, action) => {
            if (action === "skip") {
              services.learning.mark("skipped");
              services.navigation.go(path, { return_path: null, error: null });
            } else
              services.navigation.go("/tutorial", {
                return: path,
                return_path: null,
                error: null,
              });
          }}
        />
      ) : (
        state.status !== "loading" && (
          <NavLink path="/login" class="button">
            {t("auth.title")}
          </NavLink>
        )
      )}
      <NavLink path="/practice" class="auth-local">
        {t("auth.local")}
      </NavLink>
    </div>
  );
}
