import type {
  DailyTimeRankingPage,
  DailyCompletionRankingPage,
} from "@liar/protocol";
import { useUi } from "../context";
import { Card } from "../atoms/Card";
export function DailyRankingsTable(
  props:
    | { mode: "time"; page: DailyTimeRankingPage }
    | { mode: "completion"; page: DailyCompletionRankingPage },
) {
  const { t, locale } = useUi(),
    number = new Intl.NumberFormat(locale);
  const time = props.mode === "time";
  return (
    <Card>
      <p class="muted">
        {t(time ? "ranking.timeHint" : "ranking.completionHint")}
      </p>
      <table>
        <caption>{t(time ? "ranking.time" : "ranking.completion")}</caption>
        <thead>
          <tr>
            <th scope="col" class="rank-position">
              {t("ranking.position")}
            </th>
            <th scope="col">{t("auth.nickname")}</th>
            <th scope="col" class="rank-mistakes">
              {t("ranking.mistakes")}
            </th>
            {time && (
              <th scope="col" class="rank-time">
                {t("ranking.elapsed")}
              </th>
            )}
          </tr>
        </thead>
        <tbody>
          {props.page.entries.map((row) => (
            <tr key={row.position}>
              <td>{number.format(row.position)}</td>
              <td>{row.nickname}</td>
              <td>{number.format(row.mistakes)}</td>
              {"elapsed_ms" in row && (
                <td class="rank-duration">
                  {Math.floor(row.elapsed_ms / 60000)}:
                  {String(Math.floor(row.elapsed_ms / 1000) % 60).padStart(
                    2,
                    "0",
                  )}
                  .{String(row.elapsed_ms % 1000).padStart(3, "0")}
                </td>
              )}
            </tr>
          ))}
        </tbody>
      </table>
      {props.page.entries.length === 0 && <p>{t("ranking.empty")}</p>}
      {props.page.has_more && <p class="muted">{t("ranking.more")}</p>}
    </Card>
  );
}
