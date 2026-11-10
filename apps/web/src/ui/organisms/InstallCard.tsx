import { useUi } from "../context";
import { useSnapshot } from "../useSnapshot";
import { Card } from "../atoms/Card";
import { Button } from "../atoms/Button";
export function InstallCard() {
  const { t, services } = useUi();
  const state = useSnapshot(services.install);
  const activity = useSnapshot(services.activity);
  const status = state.status === "available" ? "guide" : state.status;
  return (
    <section aria-labelledby="install-title">
      <h2 id="install-title">{t("install.title")}</h2>
      <Card>
        <p>{t("install.intro")}</p>
        <p>
          <output>{t(`install.${status}`)}</output>
        </p>
        {(state.status === "available" || state.status === "prompting") && (
          <Button
            variant="primary"
            disabled={
              state.status === "prompting" || activity.busy || activity.locked
            }
            onClick={() => {
              void services.install.install();
            }}
          >
            {t("install.action")}
          </Button>
        )}
        {state.feedback !== "none" &&
          (state.feedback === "failed" ? (
            <p role="alert">{t("install.failed")}</p>
          ) : (
            <p>
              <output>{t("install.dismissed")}</output>
            </p>
          ))}
        <p class="muted">{t("install.network")}</p>
      </Card>
    </section>
  );
}
