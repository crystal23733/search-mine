import { useEffect, useMemo } from "preact/hooks";
import { ConsentController } from "../../services/privacy/controller";
import { useUi } from "../context";
import { useSnapshot } from "../useSnapshot";
import { Button } from "../atoms/Button";
import { Card } from "../atoms/Card";
import { MeConsent } from "./MeConsent";
export function ServerConsentControls() {
  const { services, t } = useUi();
  const controller = useMemo(
    () => new ConsentController(services.auth, services.consentControl),
    [services],
  );
  const state = useSnapshot(controller),
    cmp = useSnapshot(services.consent),
    busy = state.status === "loading";
  useEffect(() => {
    void controller.load();
    const remove = services.navigation.subscribe(() => controller.cancel());
    return () => {
      remove();
      controller.dispose();
    };
  }, [controller, services]);
  return (
    <section aria-label={t("me.serverConsent")}>
      {state.snapshot && (
        <MeConsent value={state.snapshot} startedAt={state.startedAt} />
      )}
      {!state.snapshot && (
        <Card>
          <p role={state.status === "error" ? "alert" : "status"}>
            {t(
              state.status === "error"
                ? state.error === "conflict"
                  ? "consent.changed"
                  : state.error === "rate_limited"
                    ? "auth.error.auth_rate_limited"
                    : "me.error"
                : state.status === "unauthorized"
                  ? "me.signIn"
                  : busy
                    ? "me.loading"
                    : "me.unknown",
            )}
          </p>
        </Card>
      )}
      <div class="match-controls">
        <Button
          data-testid="consent-server-read"
          disabled={busy}
          onClick={() => {
            void controller.load();
          }}
        >
          {t("consent.serverRefresh")}
        </Button>
        <Button
          disabled={busy || !cmp.canReopen || cmp.working}
          onClick={() => {
            void controller.refresh();
          }}
        >
          {t("consent.serverVerify")}
        </Button>
        {(["analytics", "advertising", "all"] as const).map((purpose) => (
          <Button
            key={purpose}
            data-testid={`consent-withdraw-${purpose}`}
            disabled={busy}
            variant="ghost"
            onClick={() => {
              void controller.withdraw(purpose);
            }}
          >
            {t(`consent.withdraw.${purpose}`)}
          </Button>
        ))}
        {busy && (
          <Button variant="ghost" onClick={() => controller.cancel()}>
            {t("consent.cancel")}
          </Button>
        )}
      </div>
    </section>
  );
}
