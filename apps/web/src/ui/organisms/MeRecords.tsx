import type { MeRecord } from "@liar/protocol";
import { useUi } from "../context";
import { PersonalResult } from "./PersonalResult";
import { utcTime } from "../molecules/RecordPeriod";
export function MeRecords({ items }: { items: MeRecord[] }) {
  const { t, locale, services } = useUi();
  const number = (value: number) => new Intl.NumberFormat(locale).format(value);
  const elapsed = (value: number | null) =>
    value === null
      ? t("me.noTime")
      : new Intl.NumberFormat(locale, {
          style: "unit",
          unit: "second",
          maximumFractionDigits: 3,
        }).format(value / 1000);
  return (
    <ol class="me-records" data-testid="me-history-list">
      {items.map((item) => {
        const id =
          item.kind === "online"
            ? item.entry.result.match_id
            : item.entry.completion_id;
        return (
          <li key={id}>
            <details class="card me-record" data-testid="me-record">
              <summary>
                <span>
                  {item.kind === "online"
                    ? t(`match.result.${item.entry.result.result.outcome}`)
                    : `${item.entry.date} UTC`}
                </span>
                <span class="muted">
                  {item.kind === "online"
                    ? `${utcTime(locale, item.entry.retention_started_at)} (UTC) · ${t(`me.${item.entry.opponent}`)}`
                    : t(`me.trust.${item.entry.trust}`)}
                </span>
                <span>{t("me.detail")}</span>
              </summary>
              {item.kind === "online" ? (
                <>
                  <p>
                    {t("me.elapsed")}:{" "}
                    {elapsed(item.entry.result.end_elapsed_ms)}
                  </p>
                  <PersonalResult
                    historyLink={false}
                    result={item.entry.result}
                    ownName={
                      services.auth.read().account?.nickname ??
                      t("auth.nicknameUnset")
                    }
                    onAgain={() => services.navigation.go("/queue")}
                  />
                </>
              ) : (
                <dl class="me-details">
                  <div>
                    <dt>{t("me.serverTime")}</dt>
                    <dd>{elapsed(item.entry.verified_elapsed_ms)}</dd>
                  </div>
                  <div>
                    <dt>{t("me.replayTime")}</dt>
                    <dd>{elapsed(item.entry.replayed_elapsed_ms)}</dd>
                  </div>
                  <div>
                    <dt>
                      {t("match.mistakes", {
                        count: number(item.entry.mistakes),
                      })}
                    </dt>
                    <dd>{t(`me.trust.${item.entry.trust}`)}</dd>
                  </div>
                  <div>
                    <dt>
                      {t("me.updated", {
                        time: utcTime(locale, item.entry.completed_at),
                      })}
                    </dt>
                    <dd>
                      {t("result.openedSafe", {
                        count: number(item.entry.opened_safe),
                      })}
                    </dd>
                  </div>
                </dl>
              )}
            </details>
          </li>
        );
      })}
    </ol>
  );
}
