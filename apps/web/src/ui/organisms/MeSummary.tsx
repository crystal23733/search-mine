import type { MeMatchSummary } from "@liar/protocol";
import { useUi } from "../context";
import { Card } from "../atoms/Card";
export function MeSummary({ value }: { value: MeMatchSummary }) {
  const { t, locale } = useUi();
  const number = new Intl.NumberFormat(locale);
  const entries = [
    ["me.played", value.played],
    ["me.wins", value.wins],
    ["me.losses", value.losses],
    ["me.draws", value.draws],
    ["me.aborted", value.aborted],
    ["me.cancelled", value.cancelled],
  ] as const;
  return (
    <Card>
      <dl class="me-summary" data-testid="me-summary">
        {entries.map(([label, count]) => (
          <div key={label}>
            <dt>{t(label)}</dt>
            <dd>{number.format(count)}</dd>
          </div>
        ))}
        <div>
          <dt>{t("me.winRate")}</dt>
          <dd data-testid="me-win-rate">
            {value.win_rate === null
              ? "—"
              : new Intl.NumberFormat(locale, {
                  style: "percent",
                  maximumFractionDigits: 1,
                }).format(value.win_rate)}
          </dd>
        </div>
      </dl>
    </Card>
  );
}
