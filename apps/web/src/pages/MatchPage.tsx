import { useMemo, useState } from "preact/hooks";
import type { Difficulty } from "@liar/core-bridge";
import type { GameView } from "@liar/protocol";
import {
  LocalController,
  browserClock,
  localSeed,
} from "../services/local-session";
import { useLocalController } from "./useLocalController";
import { useUi } from "../ui/context";
import { Board } from "../ui/organisms/Board";
import { MatchSummary } from "../ui/organisms/MatchSummary";
import { MatchLayout } from "../ui/templates/MatchLayout";
import { Button } from "../ui/atoms/Button";
import { NavLink } from "../ui/molecules/NavLink";
const difficulties: Difficulty[] = ["easy", "normal", "hard"];
export function MatchPage() {
  const { t, services } = useUi();
  const [difficulty, setDifficulty] = useState<Difficulty>(() => {
    const value = services.navigation.current().searchParams.get("difficulty");
    return difficulties.includes(value as Difficulty)
      ? (value as Difficulty)
      : "normal";
  });
  const [round, setRound] = useState(0);
  const controller = useMemo(
    () =>
      new LocalController<GameView>(
        async () => {
          const seed = localSeed();
          const core = await services.practiceCore();
          return {
            init: () => core.init(seed, difficulty),
            advance: (time) => core.advance(time),
            step: (input, time) => core.step(input, time),
            dispose: () => core.dispose(),
          };
        },
        browserClock,
        (view) => ["finished", "aborted", "cancelled"].includes(view.phase),
      ),
    [services, difficulty, round],
  );
  const state = useLocalController(controller);
  return (
    <>
      <div class="page-title">
        <div>
          <p class="eyebrow">{t("home.practiceHint")}</p>
          <h1>{t("match.title")}</h1>
        </div>
        <NavLink path="/">{t("home")}</NavLink>
      </div>
      <div class="match-controls">
        <label>
          {t("match.difficulty")}
          <select
            value={difficulty}
            onChange={(event) => {
              const next = event.currentTarget.value as Difficulty;
              setDifficulty(next);
              services.navigation.go("/practice", { difficulty: next });
            }}
          >
            {difficulties.map((value) => (
              <option key={value} value={value}>
                {t(`match.${value}`)}
              </option>
            ))}
          </select>
        </label>
        <Button onClick={() => setRound(round + 1)}>
          {t("match.restart")}
        </Button>
        <NavLink path="/tutorial">{t("tutorial.replay")}</NavLink>
      </div>
      {state.status === "loading" && (
        <output class="status-message">{t("match.loading")}</output>
      )}
      {state.status === "error" && <p role="alert">{t("match.unavailable")}</p>}
      {state.status === "ready" && state.error && (
        <output class="status-message">{t("match.rejected")}</output>
      )}
      {state.view && (
        <MatchLayout
          summary={
            <MatchSummary
              view={state.view}
              opponent={`${t("match.bot")} · ${t(`match.${difficulty}`)}`}
            />
          }
        >
          <Board
            key={round + difficulty}
            view={state.view}
            createRenderer={services.boardRenderer}
            onAction={(action) => {
              void controller.submit(action);
            }}
          />
          <Button
            variant="primary"
            disabled={
              state.status !== "ready" ||
              state.view.phase !== "playing" ||
              state.view.own.stun_ms > 0 ||
              state.view.own.gauge < state.view.rules.rules.gauge_capacity
            }
            onClick={() => {
              void controller.submit({ type: "attack" });
            }}
          >
            {t("home.attack")}
          </Button>
        </MatchLayout>
      )}
    </>
  );
}
