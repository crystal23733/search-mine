import { useEffect, useMemo } from "preact/hooks";
import { LatestResultController } from "../services/online/latest-result";
import { useUi } from "../ui/context";
import { useSnapshot } from "../ui/useSnapshot";
import { Card } from "../ui/atoms/Card";
import { Button } from "../ui/atoms/Button";
import { NavLink } from "../ui/molecules/NavLink";
import { PersonalResult } from "../ui/organisms/PersonalResult";
export function LatestResultPage() {
  const { t, services } = useUi(),
    auth = useSnapshot(services.auth);
  if (!services.auth.connected() || !auth.account?.nickname)
    return (
      <div class="reading-page">
        <Card>
          <h1>{t("result.latest")}</h1>
          <p role={auth.status === "unavailable" ? "alert" : "status"}>
            {t(
              auth.status === "loading"
                ? "auth.loading"
                : auth.status === "unavailable"
                  ? "queue.error"
                  : "queue.signIn",
            )}
          </p>
          <div class="result-actions">
            <NavLink
              path={auth.account ? "/onboarding" : "/login"}
              class="button button--primary"
            >
              {t(auth.account ? "auth.nicknameTitle" : "auth.title")}
            </NavLink>
            <NavLink path="/">{t("home")}</NavLink>
          </div>
        </Card>
      </div>
    );
  return (
    <ResultSession
      key={`${auth.account.id}-${services.auth.revision()}`}
      nickname={auth.account.nickname}
    />
  );
}
function ResultSession({ nickname }: { nickname: string }) {
  const { t, services } = useUi();
  const controller = useMemo(
    () => new LatestResultController(services.auth, services.online),
    [services],
  );
  const state = useSnapshot(controller);
  useEffect(() => {
    void controller.start();
    return () => controller.dispose();
  }, [controller]);
  return (
    <div class="reading-page">
      <h1>{t("result.latest")}</h1>
      {state.result ? (
        <PersonalResult
          result={state.result}
          ownName={nickname}
          onAgain={() => services.navigation.go("/queue")}
        />
      ) : (
        <Card>
          <p
            role={state.status === "error" ? "alert" : "status"}
            aria-live="polite"
          >
            {t(
              state.status === "error"
                ? "result.unavailable"
                : state.status === "empty"
                  ? "result.latestEmpty"
                  : state.status === "unauthorized"
                    ? "queue.signIn"
                    : "result.checking",
            )}
          </p>
          <div class="result-actions">
            {(state.status === "error" || state.status === "empty") && (
              <Button onClick={() => void controller.start()}>
                {t("result.check")}
              </Button>
            )}
            <NavLink path="/" class="button">
              {t("home")}
            </NavLink>
          </div>
        </Card>
      )}
    </div>
  );
}
