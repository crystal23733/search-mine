import { useEffect, useMemo, useState } from "preact/hooks";
import type { RoomView, RoundResult } from "@liar/protocol";
import type { RequestOwner } from "../services/auth/requests";
import type { RoomPort } from "../services/rooms/types";
import { RoundController } from "../services/rooms/round-controller";
import { useUi } from "../ui/context";
import { useSnapshot } from "../ui/useSnapshot";
import { Board } from "../ui/organisms/Board";
import { Card } from "../ui/atoms/Card";
import { Button } from "../ui/atoms/Button";
import { MatchLayout } from "../ui/templates/MatchLayout";

export function RoomRound({
  room,
  port,
  owner,
}: {
  room: RoomView;
  port: RoomPort;
  owner: RequestOwner;
}) {
  const { t, services } = useUi();
  const roomId = room.room_id;
  const roundId = room.round!.round_id;
  const seat = room.round!.seat;
  const controller = useMemo(
    () =>
      new RoundController(services.auth, port, owner, {
        roomId,
        roundId,
        seat,
      }),
    [
      services.auth,
      port,
      owner.accountId,
      owner.revision,
      roomId,
      roundId,
      seat,
    ],
  );
  const state = useSnapshot(controller);
  const [locked, setLocked] = useState(false);
  const [now, setNow] = useState(() => performance.now());
  useEffect(() => {
    let release: () => void;
    try {
      release = services.activity.hold();
    } catch {
      setLocked(true);
      return;
    }
    controller.start();
    return () => {
      controller.dispose();
      release();
    };
  }, [services.activity, controller]);
  useEffect(() => {
    const timer = setInterval(() => setNow(performance.now()), 200);
    return () => clearInterval(timer);
  }, []);
  const { view } = state;
  if (
    locked ||
    state.status === "disconnected" ||
    state.status === "reconnecting"
  )
    return <output>{t("queue.disconnected")}</output>;
  if (!view) return <output>{t("rooms.countdown")}</output>;
  if (view.result)
    return (
      <RoundStandings
        result={view.result}
        seat={seat}
        failed={state.recording === "failed"}
      />
    );
  const elapsed = Math.max(0, now - state.receivedAt);
  const time = Math.max(0, Math.ceil((view.remaining_ms - elapsed) / 1000));
  const seconds = Math.max(0, Math.ceil((view.countdown_ms - elapsed) / 1000));
  return (
    <MatchLayout
      summary={
        <Card>
          <h2>{t("rooms.opponents")}</h2>
          {view.phase === "countdown" ? (
            <output>{t("match.countdown", { seconds })}</output>
          ) : (
            <p>
              {t("match.time")} {time}
            </p>
          )}
          <ul class="rooms-list">
            {view.opponents.map((opponent) => (
              <li key={opponent.seat} class="rooms-row">
                <strong>{opponent.nickname ?? t("rooms.anonymous")}</strong>
                <span>
                  {opponent.forfeited
                    ? t("rooms.dnf")
                    : `${t("daily.progress")} ${opponent.progress.opened_safe}`}
                  {!opponent.forfeited &&
                    opponent.progress.reconnect_ms !== null && (
                      <span class="rooms-opponent-status">
                        {t("match.opponentReconnecting", {
                          seconds: Math.max(
                            0,
                            Math.ceil(
                              (opponent.progress.reconnect_ms - elapsed) / 1000,
                            ),
                          ),
                        })}
                      </span>
                    )}
                  {!opponent.forfeited &&
                    opponent.progress.stun_ms > elapsed && (
                      <span class="rooms-opponent-status">
                        {t("board.stun", {
                          seconds: Math.ceil(
                            (opponent.progress.stun_ms - elapsed) / 1000,
                          ),
                        })}
                      </span>
                    )}
                </span>
              </li>
            ))}
          </ul>
          <p>
            {t("match.gauge")} {view.own.gauge}/
            {view.rules.rules.gauge_capacity}
          </p>
          {state.rejected && <output>{t("match.rejected")}</output>}
        </Card>
      }
    >
      <Board
        view={view}
        createRenderer={services.boardRenderer}
        onAction={(action) => controller.send(action)}
        disabled={view.forfeited || state.status !== "ready"}
      />
      <Button
        variant="primary"
        disabled={
          view.phase !== "playing" ||
          view.own.stun_ms > 0 ||
          view.own.gauge < view.rules.rules.gauge_capacity ||
          view.forfeited
        }
        onClick={() => controller.send({ type: "attack" })}
      >
        {t("home.attack")}
      </Button>
    </MatchLayout>
  );
}

export function RoundStandings({
  result,
  seat,
  failed,
}: {
  result: RoundResult;
  seat: number;
  failed: boolean;
}) {
  const { t } = useUi();
  return (
    <Card class="rooms-results">
      <h2>{t("rooms.results")}</h2>
      <p>
        {t(
          result.reason === "abandoned"
            ? "match.result.abort"
            : `match.reason.${result.reason}`,
        )}
      </p>
      {failed && <p role="alert">{t("queue.recording.failed")}</p>}
      <ul class="rooms-list">
        {result.standings.map((entry) => (
          <li class="rooms-row" key={entry.seat}>
            <span>
              <strong>{entry.nickname ?? t("rooms.anonymous")}</strong>
              {entry.seat === seat && ` (${t("match.you")})`}
            </span>
            <span>
              {entry.forfeited
                ? t("rooms.dnf")
                : entry.rank === null
                  ? ""
                  : t(
                      result.standings.filter(
                        (standing) => standing.rank === entry.rank,
                      ).length > 1
                        ? "rooms.tied"
                        : "rooms.rank",
                      { rank: entry.rank },
                    )}
            </span>
            <span>
              {t("daily.progress")} {entry.opened_safe}
            </span>
          </li>
        ))}
      </ul>
    </Card>
  );
}
