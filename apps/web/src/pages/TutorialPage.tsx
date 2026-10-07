import { useEffect, useMemo, useState } from "preact/hooks";
import type { TutorialView } from "@liar/protocol";
import { LocalController, browserClock } from "../services/local-session";
import { useLocalController } from "./useLocalController";
import { useUi } from "../ui/context";
import { Board } from "../ui/organisms/Board";
import { MatchSummary } from "../ui/organisms/MatchSummary";
import { MatchLayout } from "../ui/templates/MatchLayout";
import { Button } from "../ui/atoms/Button";
import { Card } from "../ui/atoms/Card";
import { coordinate } from "../board/presentation";
const returnPaths = new Set(["/", "/practice", "/friends", "/daily", "/queue"]);
export function TutorialPage() {
  const { t, services } = useUi();
  const [round, setRound] = useState(0);
  const controller = useMemo(
    () =>
      new LocalController<TutorialView>(
        () => services.trainingCore(),
        browserClock,
        (view) => view.stage === "complete",
      ),
    [services, round],
  );
  const state = useLocalController(controller);
  useEffect(() => {
    if (state.view?.stage === "complete") services.learning.mark("complete");
  }, [state.view?.stage, services]);
  const leave = (skip: boolean) => {
    if (skip && services.learning.read() !== "complete")
      services.learning.mark("skipped");
    const path =
      services.navigation.current().searchParams.get("return") ?? "/";
    services.navigation.go(returnPaths.has(path) ? path : "/", {
      return: null,
    });
  };
  const view = state.view,
    expected = view?.expected;
  const cell =
    expected && "cell" in expected && view
      ? coordinate(expected.cell, view.game.rules.rules.width)
      : "";
  return (
    <>
      <div class="page-title">
        <div>
          <p class="eyebrow">{t("tutorial.eyebrow")}</p>
          <h1>{t("tutorial.title")}</h1>
        </div>
        <Button variant="ghost" onClick={() => leave(true)}>
          {t("tutorial.skip")}
        </Button>
      </div>
      <p>{t("tutorial.trainingRules")}</p>
      {!services.learning.persistent() && (
        <output class="status-message">{t("tutorial.memory")}</output>
      )}
      {state.status === "loading" && (
        <output class="status-message">{t("match.loading")}</output>
      )}
      {state.status === "error" && (
        <>
          <p role="alert">{t("match.unavailable")}</p>
          <Button onClick={() => setRound(round + 1)}>
            {t("match.restart")}
          </Button>
        </>
      )}
      {state.error && state.status === "ready" && (
        <output class="status-message">{t("tutorial.wrongAction")}</output>
      )}
      {view && (
        <MatchLayout
          guide={
            <Card class="lesson-card">
              <h2 aria-live="polite">{t(`tutorial.${view.stage}.title`)}</h2>
              <p>{t(`tutorial.${view.stage}.hint`, { cell })}</p>
              {expected && view.stage !== "complete" && (
                <Button
                  variant="primary"
                  onClick={() => {
                    void controller.submit(expected);
                  }}
                >
                  {t(`tutorial.${view.stage}.action`, { cell })}
                </Button>
              )}
              {view.stage === "complete" && (
                <>
                  <Button variant="primary" onClick={() => leave(false)}>
                    {t("tutorial.continue")}
                  </Button>
                  <Button onClick={() => setRound(round + 1)}>
                    {t("tutorial.replay")}
                  </Button>
                </>
              )}
            </Card>
          }
          summary={<MatchSummary view={view.game} opponent={t("match.bot")} />}
        >
          <Board
            key={round}
            view={view.game}
            createRenderer={services.boardRenderer}
            onAction={(action) => {
              void controller.submit(action);
            }}
          />
        </MatchLayout>
      )}
    </>
  );
}
