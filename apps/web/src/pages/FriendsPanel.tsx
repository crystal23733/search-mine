import { useEffect, useRef, useState } from "preact/hooks";
import type { MessageKey } from "../services/i18n";
import { roomCode, roomInvitation } from "../services/online/invitation";
import type { OnlinePanelProps } from "./OnlineEntry";
import { useUi } from "../ui/context";
import { Button } from "../ui/atoms/Button";
import { Card } from "../ui/atoms/Card";
import { NavLink } from "../ui/molecules/NavLink";
export function FriendsPanel({
  controller,
  state,
  locked,
  nickname,
}: OnlinePanelProps) {
  const { t, locale, services } = useUi();
  const incoming = services.navigation.current().searchParams.get("code") ?? "";
  const [value, setValue] = useState(
    roomCode(incoming) ?? incoming.slice(0, 64),
  );
  const [notice, setNotice] = useState<{
    roomId: string;
    key: MessageKey;
  } | null>(null);
  const [seenRoom, setSeenRoom] = useState(false);
  const lobby = state.lobby?.state;
  const room = lobby?.type === "room" ? lobby : null;
  const current = useRef<string | null>(null);
  const operation = useRef(0);
  current.current = room?.room_id ?? null;
  useEffect(() => {
    setValue(roomCode(incoming) ?? incoming.slice(0, 64));
  }, [incoming]);
  useEffect(() => {
    if (room) setSeenRoom(true);
  }, [room?.room_id]);
  useEffect(
    () => () => {
      current.current = null;
    },
    [],
  );
  const share = async (kind: "copy" | "nativeShare") => {
    if (!room || state.working || state.status !== "waiting") return;
    const id = room.room_id;
    const sequence = ++operation.current;
    setNotice(null);
    try {
      await services.share[kind](
        roomInvitation(services.navigation.current().origin, locale, room.code),
      );
      if (current.current === id && operation.current === sequence)
        setNotice(
          kind === "copy" ? { roomId: id, key: "friends.copied" } : null,
        );
    } catch {
      if (current.current === id && operation.current === sequence)
        setNotice({
          roomId: id,
          key: kind === "copy" ? "friends.copyError" : "friends.shareError",
        });
    }
  };
  const disabled = locked || state.working || state.status !== "ready";
  const number = (n: number) =>
    new Intl.NumberFormat(locale, { minimumIntegerDigits: 2 }).format(n);
  if (!room && state.status === "error") return null;
  return (
    <div class="friends-panel">
      {room ? (
        <>
          <Card class="friend-code-card">
            <h2>{t("friends.code")}</h2>
            <output
              class="friend-code"
              data-testid="room-code"
              aria-label={t("friends.code")}
            >
              {room.code}
            </output>
            <p>
              <output aria-live="off">
                {t("friends.expires", {
                  time: `${number(Math.floor(state.roomMs / 60000))}:${number(Math.floor(state.roomMs / 1000) % 60)}`,
                })}
              </output>
            </p>
            <div class="friend-actions">
              <Button
                disabled={state.working || state.status !== "waiting"}
                onClick={() => void share("copy")}
              >
                {t("friends.copy")}
              </Button>
              <Button
                variant="primary"
                disabled={state.working || state.status !== "waiting"}
                onClick={() => void share("nativeShare")}
              >
                {t("friends.share")}
              </Button>
            </div>
            {notice?.roomId === room.room_id && (
              <output>{t(notice.key)}</output>
            )}
          </Card>
          <section
            aria-label={t("friends.seats", {
              count: room.occupied.filter(Boolean).length,
            })}
          >
            <h2>
              {t("friends.seats", {
                count: room.occupied.filter(Boolean).length,
              })}
            </h2>
            <ol class="friend-seats">
              {room.occupied.map((occupied, index) => (
                <li
                  key={index}
                  data-testid={`room-seat-${index}`}
                  class={
                    occupied ? "friend-seat" : "friend-seat friend-seat--empty"
                  }
                >
                  <span>
                    {index === room.own_seat
                      ? `${nickname} · ${t("match.you")}`
                      : occupied
                        ? t("queue.human")
                        : t("friends.waiting")}
                  </span>
                  {occupied && (
                    <strong>
                      {t(
                        room.ready[index]
                          ? "friends.ready"
                          : "friends.notReady",
                      )}
                    </strong>
                  )}
                </li>
              ))}
            </ol>
          </section>
          <Button
            variant="primary"
            disabled={state.working || locked || state.status !== "waiting"}
            aria-pressed={room.ready[room.own_seat]}
            onClick={() => void controller.setReady(!room.ready[room.own_seat])}
          >
            {t(room.ready[room.own_seat] ? "friends.unready" : "friends.ready")}
          </Button>
          <Button
            disabled={state.working || state.status !== "waiting"}
            onClick={() => void controller.cancel()}
          >
            {t("friends.leave")}
          </Button>
        </>
      ) : state.status === "ready" ? (
        <Card>
          {seenRoom && <output>{t("friends.closed")}</output>}
          <Button
            variant="primary"
            disabled={disabled}
            onClick={() => void controller.createRoom()}
          >
            {t("friends.create")}
          </Button>
          <form
            class="friend-join"
            onSubmit={(e) => {
              e.preventDefault();
              void controller.joinRoom(value);
            }}
          >
            <label for="friend-code-input">{t("friends.code")}</label>
            <p id="friend-code-hint">{t("friends.joinHint")}</p>
            <input
              id="friend-code-input"
              name="code"
              type="text"
              value={value}
              maxLength={64}
              autoComplete="off"
              autoCapitalize="characters"
              spellcheck={false}
              aria-describedby="friend-code-hint"
              aria-invalid={value.length > 0 && !roomCode(value)}
              disabled={disabled}
              onInput={(e) => setValue(e.currentTarget.value)}
            />
            <Button type="submit" disabled={disabled || !roomCode(value)}>
              {t("friends.join")}
            </Button>
          </form>
          <p>{t("friends.explicitJoin")}</p>
        </Card>
      ) : (
        <Card>
          <output class="status-message">
            {t(
              state.status === "connecting"
                ? "queue.connecting"
                : state.status === "waiting"
                  ? "queue.preparing"
                  : "match.loading",
            )}
          </output>
          {lobby?.type === "queued" && (
            <>
              <p>{t("friends.otherQueue")}</p>
              <NavLink path="/queue">{t("queue.title")}</NavLink>
            </>
          )}
          {state.status === "waiting" && (
            <Button
              disabled={state.working}
              onClick={() => void controller.cancel()}
            >
              {t("queue.cancel")}
            </Button>
          )}
        </Card>
      )}
    </div>
  );
}
