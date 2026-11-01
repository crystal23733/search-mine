import { useEffect, useMemo } from "preact/hooks";
import type { MeKind, MeOpponentFilter, MeOverview } from "@liar/protocol";
import { MeController } from "../../services/me/controller";
import { useUi } from "../context";
import { useSnapshot } from "../useSnapshot";
import { Card } from "../atoms/Card";
import { Button } from "../atoms/Button";
import { NavLink } from "../molecules/NavLink";
import { RecordPeriod, utcTime } from "../molecules/RecordPeriod";
import { MeSummary } from "./MeSummary";
import { MeRecords } from "./MeRecords";
import { MeConsent } from "./MeConsent";
import { AccountSettings } from "./AccountSettings";
import { isProvider } from "../../services/auth/types";
import { providerName } from "../molecules/ProviderButton";
export type MeSection = "overview" | "history" | "account" | "privacy";
export function MeContent({ section }: { section: MeSection }) {
  const { t, locale, services } = useUi();
  const controller = useMemo(
    () =>
      new MeController(
        services.auth,
        services.me,
        section === "history" ? "history" : "overview",
      ),
    [services, section],
  );
  const state = useSnapshot(controller);
  useEffect(() => {
    void controller.start();
    return () => controller.dispose();
  }, [controller]);
  const value = state.overview;
  const page = state.history;
  return (
    <>
      {section === "history" && (
        <div class="me-filters">
          <label>
            {t("me.kind")}
            <select
              value={state.kind}
              onChange={(event) => {
                void controller.filter(
                  event.currentTarget.value as MeKind,
                  state.opponent,
                );
              }}
            >
              <option value="online">{t("me.online")}</option>
              <option value="daily">{t("me.daily")}</option>
            </select>
          </label>
          {state.kind === "online" && (
            <label>
              {t("me.opponent")}
              <select
                value={state.opponent}
                onChange={(event) => {
                  void controller.filter(
                    state.kind,
                    event.currentTarget.value as MeOpponentFilter,
                  );
                }}
              >
                {(["all", "human", "bot"] as const).map((option) => (
                  <option key={option} value={option}>
                    {t(`me.${option}`)}
                  </option>
                ))}
              </select>
            </label>
          )}
        </div>
      )}
      {(state.status === "loading" ||
        state.status === "idle" ||
        state.status === "error" ||
        state.status === "unauthorized") && (
        <Card>
          <p role={state.status === "error" ? "alert" : "status"}>
            {t(
              state.status === "error"
                ? state.error === "rate_limited"
                  ? "auth.error.auth_rate_limited"
                  : "me.error"
                : state.status === "unauthorized"
                  ? "me.signIn"
                  : "me.loading",
            )}
          </p>
          {state.status === "error" && (
            <Button
              onClick={() => {
                void controller.start();
              }}
            >
              {t("me.retry")}
            </Button>
          )}
        </Card>
      )}
      {section === "overview" && value && <Overview value={value} />}
      {section === "history" && page && (
        <>
          <RecordPeriod from={page.window_started_at} to={page.observed_at} />
          <p>
            {t("me.total", {
              count: new Intl.NumberFormat(locale).format(page.total),
            })}
          </p>
          {page.online && <MeSummary value={page.online} />}
          {page.items.length === 0 ? (
            <output>{t("me.empty")}</output>
          ) : (
            <MeRecords items={page.items} />
          )}
        </>
      )}
      {section === "history" && (
        <div class="match-controls">
          <Button
            disabled={state.page === 0 || state.status === "loading"}
            onClick={() => {
              void controller.back();
            }}
          >
            {t("me.back")}
          </Button>
          <Button
            disabled={!page?.next || state.status !== "ready"}
            onClick={() => {
              void controller.next();
            }}
          >
            {t("me.next")}
          </Button>
        </div>
      )}
      {section === "account" && (
        <>
          {value && (
            <Card>
              <dl class="me-details">
                <div>
                  <dt>{t("me.created")}</dt>
                  <dd>{utcTime(locale, value.account.created_at)} (UTC)</dd>
                </div>
                <div>
                  <dt>{t("me.lastSeen")}</dt>
                  <dd>{utcTime(locale, value.account.last_seen_at)} (UTC)</dd>
                </div>
              </dl>
              <ul class="me-identities">
                {value.account.identities.map((identity) => (
                  <li key={identity.provider}>
                    {isProvider(identity.provider)
                      ? providerName(t, identity.provider)
                      : t("me.unknown")}{" "}
                    ·{" "}
                    {t("me.linked", {
                      time: utcTime(locale, identity.linked_at),
                    })}
                  </li>
                ))}
              </ul>
            </Card>
          )}
          <AccountSettings />
        </>
      )}
      {section === "privacy" && (
        <>
          {value && <MeConsent value={value} startedAt={state.startedAt} />}
          <h2>{t("me.storage")}</h2>
          <Card>
            <p>{t("me.retention")}</p>
          </Card>
        </>
      )}
      {(section === "history" || section === "overview") && (
        <p class="muted">
          {t("me.localHint")}{" "}
          <NavLink path="/daily">{t("me.localRecords")}</NavLink>
        </p>
      )}
    </>
  );
}
function Overview({ value }: { value: MeOverview }) {
  const { t } = useUi();
  return (
    <>
      <Card>
        <strong>{value.account.nickname ?? t("auth.nicknameUnset")}</strong>
      </Card>
      <RecordPeriod from={value.window_started_at} to={value.observed_at} />
      <h2>{t("me.online")}</h2>
      <MeSummary value={value.online} />
      <h3>{t("me.recent")}</h3>
      {value.recent_online.length ? (
        <MeRecords
          items={value.recent_online.map((entry) => ({
            kind: "online",
            entry,
          }))}
        />
      ) : (
        <p>{t("me.empty")}</p>
      )}
      <h2>{t("me.daily")}</h2>
      <p>{t("me.total", { count: value.daily_total })}</p>
      {value.recent_daily.length ? (
        <MeRecords
          items={value.recent_daily.map((entry) => ({ kind: "daily", entry }))}
        />
      ) : (
        <p>{t("me.empty")}</p>
      )}
      <NavLink path="/me/history" class="button">
        {t("me.allRecords")}
      </NavLink>
    </>
  );
}
