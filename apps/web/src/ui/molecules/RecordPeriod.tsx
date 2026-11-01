import type { Locale } from "../../services/locale";
import { useUi } from "../context";
export function utcTime(locale: Locale, seconds: number) {
  return new Intl.DateTimeFormat(locale, {
    dateStyle: "medium",
    timeStyle: "short",
    timeZone: "UTC",
  }).format(new Date(seconds * 1000));
}
export function RecordPeriod({ from, to }: { from: number; to: number }) {
  const { t, locale } = useUi();
  return (
    <div class="me-period">
      <p>{t("me.periodHint")}</p>
      <p>
        {t("me.period", {
          from: utcTime(locale, from),
          to: utcTime(locale, to),
        })}
      </p>
      <p class="muted">{t("me.updated", { time: utcTime(locale, to) })}</p>
    </div>
  );
}
