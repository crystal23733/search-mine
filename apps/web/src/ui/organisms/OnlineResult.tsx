import { useState } from "preact/hooks";
import type { GameView, RecordingStatus } from "@liar/protocol";
import { useUi } from "../context";
import { Card } from "../atoms/Card";
import { Button } from "../atoms/Button";
import { NavLink } from "../molecules/NavLink";
import { Board } from "./Board";
export function OnlineResult({
  view,
  ownName,
  opponent,
  recording,
  onAgain,
}: {
  view: GameView;
  ownName: string;
  opponent: string;
  recording: RecordingStatus | null;
  onAgain(): void;
}) {
  const { t, locale, services } = useUi(),
    [expanded, setExpanded] = useState(false);
  const result = view.result;
  if (!result) return null;
  const number = (value: number) => new Intl.NumberFormat(locale).format(value);
  const safe = view.own.cells.filter((c) => c.state === "safe").length,
    total =
      view.rules.rules.width * view.rules.rules.height - view.rules.rules.mines;
  return (
    <div class="online-result">
      <p class="eyebrow">{t("queue.online")}</p>
      <h2 aria-live="polite">{t(`match.result.${result.outcome}`)}</h2>
      <p>{t(`match.reason.${result.reason}`)}</p>
      <Card>
        <p>
          {ownName}{" "}
          <strong>
            {number(safe)} / {number(total)}
          </strong>
        </p>
        <progress aria-label={ownName} value={safe} max={total} />
        <p>
          {opponent}{" "}
          <strong>
            {number(view.opponent.opened_safe)} / {number(total)}
          </strong>
        </p>
        <progress
          aria-label={opponent}
          value={view.opponent.opened_safe}
          max={total}
        />
      </Card>
      <Card>
        <p>{t("match.mistakes", { count: view.own.stats.mistakes })}</p>
        <p>
          {t("match.accusations", {
            correct: view.own.stats.correct_accusations,
            total: view.own.stats.accusation_attempts,
          })}
        </p>
        <output data-testid="recording">
          {t(`queue.recording.${recording ?? "pending"}`)}
        </output>
      </Card>
      <div class="result-actions">
        <Button variant="primary" onClick={onAgain}>
          {t("queue.again")}
        </Button>
        <NavLink path="/" class="button">
          {t("home")}
        </NavLink>
      </div>
      <details onToggle={(event) => setExpanded(event.currentTarget.open)}>
        <summary>{t("board.title")}</summary>
        {expanded && (
          <Board
            disabled
            view={view}
            createRenderer={services.boardRenderer}
            onAction={() => {}}
          />
        )}
      </details>
    </div>
  );
}
