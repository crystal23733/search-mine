import { useEffect, useLayoutEffect, useMemo, useState } from "preact/hooks";
import type { RequestOwner } from "../services/auth/requests";
import { OfficialDailyController } from "../services/daily/official";
import { browserClock } from "../services/local-session";
import {
  dailyShare,
  verifiedDailyShare,
  shareMosaic,
  shareText,
} from "../services/daily-share";
import { useUi } from "../ui/context";
import { useSnapshot } from "../ui/useSnapshot";
import { Card } from "../ui/atoms/Card";
import { Button } from "../ui/atoms/Button";
import { Dialog } from "../ui/atoms/Dialog";
import { NavLink } from "../ui/molecules/NavLink";
import { Board } from "../ui/organisms/Board";
import { MatchLayout } from "../ui/templates/MatchLayout";
export function OfficialDailyPage() {
  const { services, t, locale } = useUi();
  const controller = useMemo(
    () =>
      new OfficialDailyController({
        auth: services.auth,
        api: services.dailyChallenge,
        core: services.dailyCore,
        records: services.dailyRecords,
        activity: services.activity,
        clock: browserClock,
        wallClock: services.wallClock,
      }),
    [services],
  );
  const state = useSnapshot(controller),
    auth = useSnapshot(services.auth),
    offline = useSnapshot(services.offline),
    activity = useSnapshot(services.activity);
  const [confirmation, setConfirmation] = useState<
    (RequestOwner & { nickname: string }) | null
  >(null);
  const [sharing, setSharing] = useState<"idle" | "busy" | "done" | "error">(
    "idle",
  );
  const revision = services.auth.revision();
  useLayoutEffect(() => {
    controller.activate();
    return () => controller.dispose();
  }, [controller]);
  useLayoutEffect(
    () => controller.setOnline(offline.online),
    [controller, offline.online],
  );
  useEffect(() => {
    setConfirmation(null);
    setSharing("idle");
  }, [auth.account?.id, revision, state.receipt, state.view?.metadata]);
  const busy = [
    "loading",
    "playing",
    "saving",
    "submitting",
    "pending",
  ].includes(state.status);
  const available =
    auth.status === "ready" &&
    !!auth.account?.nickname &&
    services.auth.connected() &&
    offline.online &&
    !busy &&
    !activity.busy &&
    !activity.locked;
  const format = (ms: number) =>
    `${Math.floor(ms / 60000)}:${String(Math.floor(ms / 1000) % 60).padStart(2, "0")}.${String(ms % 1000).padStart(3, "0")}`;
  const result = controller.result();
  const card = result
    ? verifiedDailyShare(result.receipt, result.trust, result.rules)
    : state.view
      ? dailyShare(state.view)
      : null;
  const summary = card
    ? `Liar Sweeper · ${card.date} UTC · v${card.version}\n${t(card.completed ? "daily.complete" : "daily.incomplete")} · ${t(card.trust === "unverified" ? "daily.unverified" : card.trust === "verified_time" ? "official.verifiedTime" : "official.verifiedCompletion")}\n${card.opened_safe}/${card.safe_total} · ${t(card.trust === "verified_time" ? "ranking.elapsed" : "daily.personalTime")}: ${format(card.elapsed_ms)}\n${t("match.mistakes", { count: card.mistakes })}`
    : "";
  const share = (action: "copy" | "nativeShare" | "download") => {
    if (!card || sharing === "busy") return;
    const current = controller.result();
    if (card.trust !== "unverified" && !current) return;
    setSharing("busy");
    const generation = state.generation,
      ownerRevision = services.auth.revision();
    const currentShare = () =>
      controller.read().generation === generation &&
      services.auth.revision() === ownerRevision;
    const task =
      action === "download"
        ? services.share.download(card, summary)
        : services.share[action](shareText(card, summary));
    void task
      .then(() => {
        if (currentShare()) setSharing("done");
      })
      .catch(() => {
        if (currentShare()) setSharing("error");
      });
  };
  return (
    <>
      <div class="page-title">
        <h1>{t("official.title")}</h1>
        <NavLink path="/daily">{t("official.local")}</NavLink>
      </div>
      <Card>
        <p>{t("official.hint")}</p>
        {auth.account?.nickname ? (
          <p>{t("official.account", { name: auth.account.nickname })}</p>
        ) : (
          <NavLink path="/login" query={{ return: "daily" }}>
            {t("official.login")}
          </NavLink>
        )}
        <div class="match-controls">
          <Button
            disabled={!available}
            onClick={() => {
              const a = services.auth.read().account;
              if (a?.nickname)
                setConfirmation({
                  accountId: a.id,
                  revision: services.auth.revision(),
                  nickname: a.nickname,
                });
            }}
          >
            {t("official.start")}
          </Button>
          <Button
            disabled={!available}
            onClick={() => {
              setSharing("idle");
              void controller.inspect();
            }}
          >
            {t("official.own")}
          </Button>
          <NavLink path="/daily/rankings">{t("ranking.title")}</NavLink>
        </div>
        {state.status === "idle" && <p>{t("official.idle")}</p>}
        {state.status === "loading" && <output>{t("match.loading")}</output>}
        {state.status === "saving" && <output>{t("official.saving")}</output>}
        {state.status === "submitting" && (
          <output>{t("official.submitting")}</output>
        )}
        {state.status === "error" && (
          <p role="alert">{t("official.unavailable")}</p>
        )}
        {state.status === "interrupted" && (
          <p role="alert">{t("official.interrupted")}</p>
        )}
        {state.status === "empty" && <output>{t("official.empty")}</output>}
        {state.status === "incomplete" && (
          <output>{t("daily.incomplete")}</output>
        )}
        {state.status === "pending" && (
          <>
            <output>{t("official.pending")}</output>
            <Button
              disabled={!offline.online || !services.auth.connected()}
              onClick={() => void controller.retry()}
            >
              {t("official.retry")}
            </Button>
          </>
        )}
        {state.warning && <output>{t("daily.memory")}</output>}
      </Card>
      {state.view && (
        <MatchLayout
          guide={
            <Card>
              <h2>{t("daily.solo")}</h2>
              <p>{t("daily.rules")}</p>
              <p>
                {t(
                  result
                    ? result.trust === "verified_time"
                      ? "official.verifiedTime"
                      : "official.verifiedCompletion"
                    : state.timing
                      ? "official.timingPending"
                      : "official.completionOnly",
                )}
              </p>
            </Card>
          }
          summary={
            <Card>
              <p>
                <time dateTime={state.view.metadata.date}>
                  {state.view.metadata.date}
                </time>{" "}
                UTC
              </p>
              <p>
                {t("daily.personalTime")}:{" "}
                <output aria-label={t("daily.personalTime")}>
                  {format(state.view.elapsed_ms)}
                </output>
              </p>
              <p>
                {t("match.mistakes", { count: state.view.own.stats.mistakes })}
              </p>
              <p>{t("daily.unverifiedHint")}</p>
              {state.view.completed && <h2>{t("daily.complete")}</h2>}
            </Card>
          }
        >
          <Board
            key={state.generation}
            view={state.view}
            allowedModes={state.view.available_actions}
            createRenderer={services.boardRenderer}
            onAction={(action) => {
              void controller.submit(action);
            }}
          />
        </MatchLayout>
      )}
      {state.status === "result" && result && (
        <Card>
          <h2>{t("official.result")}</h2>
          <p>
            <time dateTime={result.receipt.metadata.date}>
              {result.receipt.metadata.date}
            </time>{" "}
            UTC
          </p>
          <p>
            {t(
              state.disposition === "practice"
                ? "official.practice"
                : "official.first",
            )}
          </p>
          <p>
            {t(
              result.trust === "verified_time"
                ? "official.verifiedTime"
                : "official.verifiedCompletion",
            )}
          </p>
          <p>
            {t("match.mistakes", {
              count: new Intl.NumberFormat(locale).format(
                result.receipt.mistakes,
              ),
            })}
          </p>
          {result.receipt.verified_elapsed_ms !== null ? (
            <p>
              {t("ranking.elapsed")}:{" "}
              <output aria-label={t("ranking.elapsed")}>
                {format(result.receipt.verified_elapsed_ms)}
              </output>
            </p>
          ) : (
            <p>{t("official.noTime")}</p>
          )}
        </Card>
      )}
      {card && (
        <Card>
          <h2>{t("daily.share")}</h2>
          <pre class="daily-mosaic" aria-label={t("daily.decorative")}>
            {shareMosaic(card).join("\n")}
          </pre>
          <p>{t("daily.shareHint")}</p>
          <div class="match-controls">
            <Button
              disabled={
                sharing === "busy" ||
                state.status === "saving" ||
                state.status === "submitting"
              }
              onClick={() => share("copy")}
            >
              {t("daily.copy")}
            </Button>
            <Button
              disabled={
                sharing === "busy" ||
                state.status === "saving" ||
                state.status === "submitting"
              }
              onClick={() => share("download")}
            >
              {t("daily.png")}
            </Button>
            <Button
              disabled={
                sharing === "busy" ||
                state.status === "saving" ||
                state.status === "submitting"
              }
              onClick={() => share("nativeShare")}
            >
              {t("daily.nativeShare")}
            </Button>
          </div>
          {sharing === "done" && <output>{t("daily.shared")}</output>}
          {sharing === "error" && <output>{t("daily.shareError")}</output>}
        </Card>
      )}
      <Dialog
        open={confirmation !== null}
        focusClose
        title={t("official.start")}
        onClose={() => setConfirmation(null)}
      >
        <p>{t("official.confirm", { name: confirmation?.nickname ?? "" })}</p>
        <Button
          disabled={
            !available ||
            confirmation?.accountId !== auth.account?.id ||
            confirmation?.revision !== revision
          }
          onClick={() => {
            const owner = confirmation;
            setConfirmation(null);
            setSharing("idle");
            if (owner) void controller.start(owner);
          }}
        >
          {t("official.confirmStart")}
        </Button>
      </Dialog>
    </>
  );
}
