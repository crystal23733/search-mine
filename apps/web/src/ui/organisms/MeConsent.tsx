import { useEffect, useState } from "preact/hooks";
import type { MeOverview } from "@liar/protocol";
import { Card } from "../atoms/Card";
import { useUi } from "../context";
import { utcTime } from "../molecules/RecordPeriod";
export function MeConsent({
  value,
  startedAt,
}: {
  value: MeOverview;
  startedAt: number;
}) {
  const { t, locale } = useUi();
  const c = value.consent;
  const [tick, setTick] = useState(0);
  const deadline =
    c.expires_at === null
      ? null
      : startedAt + (c.expires_at - value.observed_at) * 1000;
  const remaining = deadline === null ? null : deadline - performance.now();
  const expired =
    c.status === "expired" ||
    (c.status === "current" && remaining !== null && remaining <= 0);
  useEffect(() => {
    if (c.status !== "current" || remaining === null || remaining <= 0) return;
    const timer = setTimeout(
      () => setTick((n) => n + 1),
      Math.min(2147483647, Math.max(1, remaining)),
    );
    return () => clearTimeout(timer);
  }, [deadline, c.status, tick]);
  return (
    <Card>
      <div data-testid="me-server-consent">
        <h2>{t("me.serverConsent")}</h2>
        <p>{t("me.serverConsentHint")}</p>
        <output>{t(`me.status.${expired ? "expired" : c.status}`)}</output>
        {!c.cmp_available && <p>{t("me.cmpUnavailable")}</p>}
        <dl class="me-details">
          {(["analytics", "advertising"] as const).map((purpose) => (
            <div key={purpose}>
              <dt>{t(`consent.${purpose}`)}</dt>
              <dd>{t(`me.choice.${expired ? "unknown" : c[purpose]}`)}</dd>
            </div>
          ))}
          <div>
            <dt>{t("me.checked")}</dt>
            <dd>
              {c.checked_at === null
                ? t("me.unknown")
                : `${utcTime(locale, c.checked_at)} (UTC)`}
            </dd>
          </div>
          <div>
            <dt>{t("me.expires")}</dt>
            <dd>
              {c.expires_at === null
                ? t("me.unknown")
                : `${utcTime(locale, c.expires_at)} (UTC)`}
            </dd>
          </div>
        </dl>
      </div>
    </Card>
  );
}
