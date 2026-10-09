import type { GameView } from "@liar/protocol";
import { useUi } from "../context";
import { Card } from "../atoms/Card";
export function MatchSummary({
  view,
  opponent,
  heading,
  ownName,
}: {
  view: GameView;
  opponent: string;
  heading?: string;
  ownName?: string;
}) {
  const { t, locale } = useUi();
  const rules = view.rules.rules;
  const safe = view.own.cells.filter((cell) => cell.state === "safe").length;
  const total = rules.width * rules.height - rules.mines;
  const seconds = Math.ceil(view.remaining_ms / 1000);
  const time = `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
  const number = (value: number) => new Intl.NumberFormat(locale).format(value);
  return (
    <>
      <Card>
        <p class="eyebrow">{heading ?? t("match.local")}</p>
        <p>
          {t("match.boardSize", {
            width: rules.width,
            height: rules.height,
            mines: rules.mines,
          })}
        </p>
        <div class="match-timer">
          <span>{t("match.time")}</span>
          <output aria-label={t("match.time")}>{time}</output>
        </div>
        {view.phase === "countdown" && (
          <output class="status-message">
            {t("match.countdown", {
              seconds: Math.ceil(view.countdown_ms / 1000),
            })}
          </output>
        )}
        <p>
          {ownName ?? t("match.you")}{" "}
          <strong data-testid="own-progress">
            {number(safe)} / {number(total)}
          </strong>
        </p>
        <progress aria-label={t("match.you")} value={safe} max={total} />
        <p>
          {opponent}{" "}
          <strong data-testid="bot-progress">
            {number(view.opponent.opened_safe)} / {number(total)}
          </strong>
        </p>
        <progress
          aria-label={opponent}
          value={view.opponent.opened_safe}
          max={total}
        />
        {view.opponent.stun_ms > 0 && (
          <p>
            {t("match.reflected", {
              seconds: Math.ceil(view.opponent.stun_ms / 1000),
            })}
          </p>
        )}
      </Card>
      <Card>
        <h2>{t("match.gauge")}</h2>
        <p>
          <strong data-testid="gauge">
            {number(view.own.gauge)} / {number(rules.gauge_capacity)}
          </strong>
        </p>
        <meter
          aria-label={t("match.gauge")}
          value={view.own.gauge}
          min={0}
          max={rules.gauge_capacity}
        />
        <p>{t("match.mistakes", { count: view.own.stats.mistakes })}</p>
        <p>
          {t("match.accusations", {
            correct: view.own.stats.correct_accusations,
            total: view.own.stats.accusation_attempts,
          })}
        </p>
      </Card>
      {view.result && (
        <Card>
          <h2 aria-live="polite">{t(`match.result.${view.result.outcome}`)}</h2>
          <p>{t(`match.reason.${view.result.reason}`)}</p>
        </Card>
      )}
    </>
  );
}
