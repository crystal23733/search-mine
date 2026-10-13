import { useEffect, useLayoutEffect, useMemo } from "preact/hooks";
import { DailyRankingsController } from "../services/daily/rankings";
import { useUi } from "../ui/context";
import { useSnapshot } from "../ui/useSnapshot";
import { Card } from "../ui/atoms/Card";
import { Button } from "../ui/atoms/Button";
import { NavLink } from "../ui/molecules/NavLink";
import { DailyRankingsTable } from "../ui/organisms/DailyRankingsTable";
export function DailyRankingsPage() {
  const { services, t } = useUi();
  const controller = useMemo(
    () =>
      new DailyRankingsController(services.dailyRankings, services.wallClock),
    [services],
  );
  const state = useSnapshot(controller),
    offline = useSnapshot(services.offline);
  useLayoutEffect(() => () => controller.dispose(), [controller]);
  useEffect(() => {
    void controller.setOnline(offline.online);
  }, [controller, offline.online]);
  return (
    <div class="reading-page daily-rankings">
      <div class="page-title">
        <h1>{t("ranking.title")}</h1>
        <NavLink path="/daily">{t("home.daily")}</NavLink>
        <NavLink path="/daily/official">{t("official.title")}</NavLink>
      </div>
      <Card>
        {state.date && (
          <p>
            <time dateTime={state.date}>{state.date}</time> UTC
          </p>
        )}
        <p>{t("ranking.hint")}</p>
        <Button
          disabled={!offline.online || state.status === "loading"}
          onClick={() => void controller.load()}
        >
          {t("ranking.refresh")}
        </Button>
        {state.status !== "ready" && (
          <p
            role={state.status === "error" ? "alert" : "status"}
            aria-live="polite"
          >
            {t(
              state.status === "error"
                ? "ranking.unavailable"
                : state.status === "offline"
                  ? "ranking.offline"
                  : "ranking.loading",
            )}
          </p>
        )}
        {state.status === "ready" && state.result?.metadata === null && (
          <output>{t("ranking.unpublished")}</output>
        )}
      </Card>
      {state.status === "ready" && state.result?.metadata && (
        <>
          <DailyRankingsTable mode="time" page={state.result.time} />
          <DailyRankingsTable
            mode="completion"
            page={state.result.completion}
          />
        </>
      )}
    </div>
  );
}
