import type { PersonalResult as StoredResult } from "@liar/protocol";
import { useUi } from "../context";
import { Card } from "../atoms/Card";
import { Button } from "../atoms/Button";
import { NavLink } from "../molecules/NavLink";
export function PersonalResult({
  result,
  ownName,
  onAgain,
  againLabel,
}: {
  result: StoredResult;
  ownName: string;
  onAgain(): void;
  againLabel?: string;
}) {
  const { t, locale } = useUi();
  const number = (value: number) => new Intl.NumberFormat(locale).format(value);
  return (
    <section class="online-result" data-testid="personal-result">
      <p class="eyebrow">{t("result.savedTitle")}</p>
      <h2 aria-live="polite">{t(`match.result.${result.result.outcome}`)}</h2>
      <p>{t(`match.reason.${result.result.reason}`)}</p>
      <Card>
        <h3>{ownName}</h3>
        <p>
          {t("result.openedSafe", { count: number(result.own.opened_safe) })}
        </p>
        <p>{t("match.mistakes", { count: number(result.own.mistakes) })}</p>
        <p>
          {t("match.accusations", {
            correct: number(result.own.correct_accusations),
            total: number(result.own.accusation_attempts),
          })}
        </p>
        <output data-testid="recording">{t("queue.recording.saved")}</output>
      </Card>
      <div class="result-actions">
        <Button variant="primary" onClick={onAgain}>
          {againLabel ?? t("queue.again")}
        </Button>
        <NavLink path="/" class="button">
          {t("home")}
        </NavLink>
      </div>
    </section>
  );
}
