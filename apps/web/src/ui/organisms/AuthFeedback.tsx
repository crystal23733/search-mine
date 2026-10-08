import type { MessageKey } from "../../services/i18n";
import { useUi } from "../context";
import { useSnapshot } from "../useSnapshot";
import { Button } from "../atoms/Button";
import { ProviderButtons } from "../molecules/ProviderButton";
export function AuthFeedback() {
  const { t, services } = useUi(),
    state = useSnapshot(services.auth);
  const notices = {
    logged_out: "auth.loggedOut",
    unlinked: "auth.unlinked",
    deleted: "auth.deleted",
  } as const;
  return (
    <div class="auth-feedback">
      {state.status === "loading" && <output>{t("auth.loading")}</output>}
      {state.error && (
        <p role="alert">{t(`auth.error.${state.error}` as MessageKey)}</p>
      )}
      {state.notice && <output>{t(notices[state.notice])}</output>}
      {state.manualAppleDisconnect && <p>{t("auth.manualApple")}</p>}
      {state.status === "unavailable" && (
        <>
          <p class="muted">{t("auth.unavailable")}</p>
          <Button
            disabled={state.working}
            onClick={() => {
              void services.auth.refresh();
            }}
          >
            {t("auth.retry")}
          </Button>
        </>
      )}
      {state.error === "reauth_required" && state.account && (
        <section class="reauth-card">
          <h3>{t("auth.reauthTitle")}</h3>
          <p>{t("auth.reauthHint")}</p>
          <ProviderButtons intent="reauth" connectedOnly />
        </section>
      )}
    </div>
  );
}
