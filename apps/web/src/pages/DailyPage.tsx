import { useEffect, useMemo, useState } from "preact/hooks";
import { useUi } from "../ui/context";
import { useLocalController } from "./useLocalController";
import { createDailyRun, utcDailyDate } from "../services/daily-session";
import { dailyId, type DailyRecord } from "../services/daily-records";
import { dailyShare, shareMosaic, shareText } from "../services/daily-share";
import { Board } from "../ui/organisms/Board";
import { MatchLayout } from "../ui/templates/MatchLayout";
import { Card } from "../ui/atoms/Card";
import { Button } from "../ui/atoms/Button";
import { NavLink } from "../ui/molecules/NavLink";
export function DailyPage() {
  const { services, t, locale } = useUi();
  const [date, setDate] = useState(() => utcDailyDate(services.wallClock()));
  const [round, setRound] = useState(0);
  const run = useMemo(
    () => createDailyRun(date, () => services.dailyCore()),
    [services, date, round],
  );
  const state = useLocalController(run.controller);
  const [records, setRecords] = useState<DailyRecord[]>([]);
  const [storageWarning, setStorageWarning] = useState(false);
  const [saved, setSaved] = useState<{ run: typeof run; inserted: boolean }>();
  const [shareStatus, setShareStatus] = useState<
    "idle" | "busy" | "done" | "error"
  >("idle");
  useEffect(() => {
    let active = true;
    void services.dailyRecords
      .list()
      .then((result) => {
        if (active) {
          setRecords(result.value);
          setStorageWarning(result.warning);
        }
      })
      .catch(() => {
        if (active) setStorageWarning(true);
      });
    return () => {
      active = false;
    };
  }, [services, run]);
  const complete = state.view?.completed ?? false;
  useEffect(() => {
    if (!complete || !state.view) return;
    let active = true;
    const view = state.view;
    void run
      .replay()
      .then((replay) =>
        services.dailyRecords.saveFirst({
          v: 1,
          id: dailyId(view.metadata),
          status: "unverified",
          attempt_id: run.attemptId(),
          metadata: view.metadata,
          elapsed_ms: view.elapsed_ms,
          mistakes: view.own.stats.mistakes,
          replay,
        }),
      )
      .then(async (result) => {
        const list = await services.dailyRecords.list();
        if (active) {
          setSaved({ run, inserted: result.value.inserted });
          setStorageWarning(result.warning || list.warning);
          setRecords(list.value);
        }
      })
      .catch(() => {
        if (active) setStorageWarning(true);
      });
    return () => {
      active = false;
    };
  }, [services, run, complete]);
  const view = state.view;
  const card = view ? dailyShare(view) : undefined;
  const formatTime = (milliseconds: number) => {
    const seconds = Math.ceil(milliseconds / 1000);
    return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
  };
  const summary = card
    ? `Liar Sweeper · ${card.date} UTC · v${card.version}\n${t(card.completed ? "daily.complete" : "daily.incomplete")} · ${t("daily.unverified")}\n${card.opened_safe}/${card.safe_total} · ${t("daily.personalTime")}: ${formatTime(card.elapsed_ms)}\n${t("match.mistakes", { count: card.mistakes })}`
    : "";
  const share = (action: "copy" | "nativeShare" | "download") => {
    if (!card || shareStatus === "busy") return;
    setShareStatus("busy");
    const task =
      action === "download"
        ? services.share.download(card, summary)
        : services.share[action](shareText(card, summary));
    void task
      .then(() => setShareStatus("done"))
      .catch(() => setShareStatus("error"));
  };
  return (
    <>
      <div class="page-title">
        <div>
          <p class="eyebrow">{t("home.practiceHint")}</p>
          <h1>{t("daily.title")}</h1>
          <p>
            <time dateTime={date}>{date}</time> UTC · {t("daily.unverified")}
          </p>
        </div>
        <NavLink path="/">{t("home")}</NavLink>
      </div>
      <div class="match-controls">
        <Button
          onClick={() => {
            setDate(utcDailyDate(services.wallClock()));
            setRound(round + 1);
            setShareStatus("idle");
          }}
        >
          {t("daily.restart")}
        </Button>
        <p class="muted">{t("daily.reset")}</p>
      </div>
      {storageWarning && <output>{t("daily.memory")}</output>}
      {state.status === "loading" && <output>{t("match.loading")}</output>}
      {state.status === "error" && <p role="alert">{t("daily.unavailable")}</p>}
      {state.status === "ready" && state.error && (
        <output>{t("match.rejected")}</output>
      )}
      {view && card && (
        <MatchLayout
          guide={
            <Card>
              <h2>{t("daily.solo")}</h2>
              <p>{t("daily.rules")}</p>
              <p>{t("daily.unverifiedHint")}</p>
            </Card>
          }
          summary={
            <>
              <Card>
                <h2>{t("daily.progress")}</h2>
                <p data-testid="own-progress">
                  {card.opened_safe} / {card.safe_total}
                </p>
                <progress
                  aria-label={t("daily.progress")}
                  value={card.opened_safe}
                  max={card.safe_total}
                />
                <div class="match-timer">
                  <span>{t("match.time")}</span>
                  <output aria-label={t("match.time")}>
                    {formatTime(view.remaining_ms)}
                  </output>
                </div>
                {view.phase === "countdown" && (
                  <output>
                    {t("match.countdown", {
                      seconds: Math.ceil(view.countdown_ms / 1000),
                    })}
                  </output>
                )}
                <p>{t("match.mistakes", { count: card.mistakes })}</p>
              </Card>
              {view.phase === "finished" && (
                <Card>
                  <h2 aria-live="polite">
                    {t(view.completed ? "daily.complete" : "daily.incomplete")}
                  </h2>
                  <p>
                    {t("daily.personalTime")}: {formatTime(view.elapsed_ms)}
                  </p>
                  <p>{t("daily.unverifiedHint")}</p>
                  {saved?.run === run && (
                    <output>
                      {t(saved.inserted ? "daily.saved" : "daily.practice")}
                    </output>
                  )}
                </Card>
              )}
            </>
          }
        >
          <Board
            key={`${date}:${round}`}
            view={view}
            allowedModes={view.available_actions}
            createRenderer={services.boardRenderer}
            onAction={(action) => {
              void run.controller.submit(action);
            }}
          />
        </MatchLayout>
      )}
      {card && (
        <Card>
          <h2>{t("daily.share")}</h2>
          <pre class="daily-mosaic" aria-label={t("daily.decorative")}>
            {shareMosaic(card).join("\n")}
          </pre>
          <p class="muted">{t("daily.shareHint")}</p>
          <div class="match-controls">
            <Button
              disabled={shareStatus === "busy"}
              onClick={() => share("copy")}
            >
              {t("daily.copy")}
            </Button>
            <Button
              disabled={shareStatus === "busy"}
              onClick={() => share("download")}
            >
              {t("daily.png")}
            </Button>
            <Button
              disabled={shareStatus === "busy"}
              onClick={() => share("nativeShare")}
            >
              {t("daily.nativeShare")}
            </Button>
          </div>
          {shareStatus === "done" && <output>{t("daily.shared")}</output>}
          {shareStatus === "error" && <output>{t("daily.shareError")}</output>}
        </Card>
      )}
      <Card>
        <h2>{t("daily.records")}</h2>
        <p>{t("daily.recordsHint")}</p>
        {records.length ? (
          <ul class="daily-records">
            {records.map((record) => (
              <li key={record.id}>
                <time dateTime={record.metadata.date}>
                  {record.metadata.date}
                </time>
                <span>
                  {formatTime(record.elapsed_ms)} ·{" "}
                  {t("match.mistakes", {
                    count: new Intl.NumberFormat(locale).format(
                      record.mistakes,
                    ),
                  })}
                </span>
                <span>{t("daily.unverified")}</span>
              </li>
            ))}
          </ul>
        ) : (
          <p>{t("daily.empty")}</p>
        )}
        <p class="muted">{t("daily.ranking")}</p>
      </Card>
    </>
  );
}
