import { useEffect, useMemo, useState } from "preact/hooks";
import type {
  RoomEntrance,
  RoomOptions,
  RoomView,
  RoomWireError,
} from "@liar/protocol";
import { RoomsController } from "../services/rooms/controller";
import { roomCode } from "../services/online/invitation";
import type { RoomCommand, RoomPort } from "../services/rooms/types";
import { useUi } from "../ui/context";
import { useSnapshot } from "../ui/useSnapshot";
import { Button } from "../ui/atoms/Button";
import { Card } from "../ui/atoms/Card";
import { NavLink } from "../ui/molecules/NavLink";
import { RoomRound, RoundStandings } from "./RoomRound";

const joinError = (error: RoomWireError) => {
  switch (error) {
    case "full":
      return "friends.full";
    case "not_found":
      return "friends.notFound";
    case "busy":
      return "friends.otherQueue";
    case "not_ready":
      return "rooms.allReady";
    case "stale":
    case "conflict":
      return "queue.stale";
    case "capacity":
    case "rate_limited":
      return "queue.limit";
    default:
      return "queue.error";
  }
};

export function RoomsPage() {
  const { t, services } = useUi();
  return services.rooms ? (
    <RoomsExperience port={services.rooms} />
  ) : (
    <Card>
      <h1>{t("rooms.title")}</h1>
      <p role="alert">{t("queue.error")}</p>
    </Card>
  );
}

function RoomsExperience({ port }: { port: RoomPort }) {
  const { t, services, locale } = useUi();
  const auth = useSnapshot(services.auth);
  const online = useSnapshot(services.offline).online;
  const controller = useMemo(
    () =>
      new RoomsController(
        services.auth,
        port,
        () =>
          services.offline.read().online &&
          document.visibilityState !== "hidden",
      ),
    [services, port],
  );
  const state = useSnapshot(controller);
  const [invite, setInvite] = useState(
    () =>
      roomCode(services.navigation.current().searchParams.get("code") ?? "") ??
      "",
  );
  const [invalidCode, setInvalidCode] = useState(false);
  const [share, setShare] = useState<"copied" | "copyError" | null>(null);
  useEffect(() => {
    controller.start();
    return () => controller.dispose();
  }, [controller]);
  useEffect(() => setShare(null), [state.room?.room_id]);

  function eligible(code: string | null = null) {
    if (!services.auth.connected() || !auth.account?.nickname) {
      services.navigation.go(auth.account ? "/onboarding" : "/login", {
        return_path: "rooms",
        code,
      });
      return false;
    }
    if (services.learning.read() === "new") {
      services.navigation.go("/tutorial", { return: "/rooms", code });
      return false;
    }
    return true;
  }

  function enter(entrance: RoomEntrance) {
    const code = entrance.kind === "invite" ? entrance.code : null;
    if (!eligible(code)) return;
    void controller
      .mutate({ type: "join", request_id: crypto.randomUUID(), entrance })
      .then((ok) => {
        if (ok) {
          setInvite("");
          services.navigation.go("/rooms", { code: null });
        }
      });
  }

  const room = state.room;
  const disabled = state.pending || !online;
  return (
    <section class="rooms-page" aria-label={t("rooms.title")}>
      <h1>{t("rooms.title")}</h1>
      <p>{t("rooms.rules")}</p>
      {!online && <output>{t("queue.disconnected")}</output>}
      {state.error && <p role="alert">{t(joinError(state.error))}</p>}
      {state.retry && (
        <Button
          disabled={disabled}
          onClick={() => void controller.mutate(state.retry!)}
        >
          {t("me.retry")}
        </Button>
      )}
      {room ? (
        <>
          <RoomLobby
            room={room}
            disabled={disabled}
            mutate={(intent) => void controller.mutate(intent)}
            share={share}
            copy={() => {
              const url = new URL(
                `/${locale}/rooms`,
                services.navigation.current().origin,
              );
              url.searchParams.set("code", room.code);
              void navigator.clipboard?.writeText(url.href).then(
                () => setShare("copied"),
                () => setShare("copyError"),
              );
              if (!navigator.clipboard) setShare("copyError");
            }}
          />
          {room.round && room.result_summary ? (
            <RoundStandings
              result={room.result_summary.result}
              seat={room.round.seat}
              failed={room.result_summary.recording === "failed"}
            />
          ) : (
            room.round &&
            auth.account && (
              <RoomRound
                key={room.round.round_id}
                room={room}
                port={port}
                owner={{
                  accountId: auth.account.id,
                  revision: services.auth.revision(),
                }}
              />
            )
          )}
        </>
      ) : (
        <>
          {!services.auth.connected() && (
            <NavLink
              path={auth.account ? "/onboarding" : "/login"}
              query={{ return_path: "rooms", code: roomCode(invite) }}
            >
              {t(auth.account ? "auth.nicknameTitle" : "auth.title")}
            </NavLink>
          )}
          <div class="rooms-actions">
            <Card>
              <h2>{t("rooms.joinCode")}</h2>
              <form
                class="rooms-fields"
                onSubmit={(event) => {
                  event.preventDefault();
                  const code = roomCode(invite);
                  setInvalidCode(code === null);
                  if (code) enter({ kind: "invite", code });
                }}
              >
                <label>
                  {t("friends.code")}
                  <input
                    value={invite}
                    maxLength={8}
                    autoComplete="off"
                    autoCapitalize="characters"
                    spellcheck={false}
                    onInput={(event) => {
                      setInvite(event.currentTarget.value);
                      setInvalidCode(false);
                    }}
                  />
                </label>
                {invalidCode && <p role="alert">{t("friends.invalid")}</p>}
                <Button type="submit" disabled={disabled}>
                  {t("friends.join")}
                </Button>
              </form>
            </Card>
            <Card>
              <h2>{t("friends.create")}</h2>
              <RoomOptionsForm
                label={t("friends.create")}
                disabled={disabled}
                suggestedTitle={auth.account?.nickname ?? undefined}
                submit={(options) => {
                  if (eligible())
                    void controller.mutate({
                      type: "create",
                      request_id: crypto.randomUUID(),
                      options,
                    });
                }}
              />
            </Card>
          </div>
          <section aria-labelledby="room-public-heading" class="rooms-public">
            <h2 id="room-public-heading">{t("rooms.publicList")}</h2>
            {state.listError && <p role="alert">{t("queue.error")}</p>}
            {!state.page && !state.listError && (
              <output>{t("match.loading")}</output>
            )}
            {state.page?.rooms.length === 0 && <p>{t("rooms.empty")}</p>}
            <ul class="rooms-list">
              {state.page?.rooms.map((item) => (
                <li key={item.room_id} class="rooms-row">
                  <span>
                    <strong>{item.title}</strong> · {item.members}/
                    {item.capacity} · {t(`rooms.${item.phase}`)}
                    {item.members === item.capacity &&
                      ` · ${t("friends.full")}`}
                  </span>
                  <Button
                    disabled={disabled || !item.accepting}
                    onClick={() =>
                      enter({ kind: "public", room_id: item.room_id })
                    }
                  >
                    {t("friends.join")}
                  </Button>
                </li>
              ))}
            </ul>
            <div class="match-controls">
              <Button disabled={!online} onClick={() => void controller.list()}>
                {t("rooms.refresh")}
              </Button>
              {state.cursor && (
                <Button onClick={() => void controller.list(null)}>
                  {t("rooms.first")}
                </Button>
              )}
              {state.page?.next && (
                <Button onClick={() => void controller.list(state.page!.next)}>
                  {t("rooms.next")}
                </Button>
              )}
            </div>
          </section>
        </>
      )}
    </section>
  );
}

function RoomOptionsForm({
  initial,
  suggestedTitle,
  label,
  disabled,
  submit,
}: {
  initial?: RoomOptions;
  suggestedTitle?: string;
  label: string;
  disabled: boolean;
  submit(options: RoomOptions): void;
}) {
  const { t } = useUi();
  const [title, setTitle] = useState(initial?.title ?? "");
  const [capacity, setCapacity] = useState(initial?.capacity ?? 4);
  const [visibility, setVisibility] = useState<RoomOptions["visibility"]>(
    initial?.visibility ?? "public",
  );
  return (
    <form
      class="rooms-fields"
      onSubmit={(event) => {
        event.preventDefault();
        submit({
          title: title.trim() || suggestedTitle || "",
          capacity,
          visibility,
        });
      }}
    >
      <label>
        {t("rooms.name")}
        <input
          required={!suggestedTitle}
          minLength={2}
          maxLength={128}
          value={title}
          placeholder={suggestedTitle}
          onInput={(event) => setTitle(event.currentTarget.value)}
        />
      </label>
      <label>
        {t("rooms.visibility")}
        <select
          value={visibility}
          onChange={(event) =>
            setVisibility(
              event.currentTarget.value === "public" ? "public" : "invite",
            )
          }
        >
          <option value="public">{t("rooms.public")}</option>
          <option value="invite">{t("rooms.invite")}</option>
        </select>
      </label>
      <label>
        {t("rooms.capacity")}
        <select
          value={capacity}
          onChange={(event) => setCapacity(Number(event.currentTarget.value))}
        >
          {[2, 3, 4].map((size) => (
            <option key={size} value={size}>
              {size}
            </option>
          ))}
        </select>
      </label>
      <Button
        type="submit"
        variant={initial ? "secondary" : "primary"}
        disabled={disabled}
      >
        {label}
      </Button>
    </form>
  );
}

function RoomLobby({
  room,
  disabled,
  mutate,
  copy,
  share,
}: {
  room: RoomView;
  disabled: boolean;
  mutate(intent: RoomCommand): void;
  copy(): void;
  share: "copied" | "copyError" | null;
}) {
  const { t } = useUi();
  const host = room.own_seat === room.host_seat;
  const ready = room.members.find(
    (member) => member.seat === room.own_seat,
  )!.ready;
  const setup = room.phase === "waiting" || room.phase === "results";
  const canStart =
    room.phase === "waiting" &&
    room.members.length >= 2 &&
    room.members.every((member) => member.ready);
  const identity = () => ({
    request_id: crypto.randomUUID(),
    room_id: room.room_id,
    revision: room.revision,
  });
  return (
    <Card class="rooms-lobby">
      <h2>{room.options.title}</h2>
      <output>
        {t(`rooms.${room.phase}`)} · {room.members.length}/
        {room.options.capacity} · {t(`rooms.${room.options.visibility}`)}
      </output>
      {room.start_error && <p role="alert">{t(joinError(room.start_error))}</p>}
      {room.phase === "playing" && !room.round && (
        <output>{t("rooms.waitNext")}</output>
      )}
      <ul class="rooms-list">
        {room.members.map((member) => (
          <li key={member.seat} class="rooms-row">
            <span>
              <strong>{member.nickname}</strong>
              {member.seat === room.own_seat && ` (${t("match.you")})`}
              {member.seat === room.host_seat && ` · ${t("rooms.host")}`}
            </span>
            <span>
              {t(member.ready ? "friends.ready" : "friends.notReady")}
            </span>
            {host && setup && member.seat !== room.own_seat && (
              <Button
                disabled={disabled}
                onClick={() =>
                  mutate({ type: "kick", ...identity(), seat: member.seat })
                }
              >
                {t("rooms.kick", { name: member.nickname })}
              </Button>
            )}
          </li>
        ))}
      </ul>
      <div class="match-controls">
        {(setup || (room.phase === "preparing" && ready)) && (
          <Button
            disabled={disabled}
            onClick={() =>
              mutate({ type: "ready", ...identity(), ready: !ready })
            }
          >
            {t(ready ? "friends.unready" : "friends.ready")}
          </Button>
        )}
        {host && setup && (
          <Button
            variant="primary"
            disabled={disabled || !canStart}
            onClick={() => mutate({ type: "start", ...identity() })}
          >
            {t("rooms.start")}
          </Button>
        )}
        <Button
          disabled={disabled}
          onClick={() => mutate({ type: "leave", ...identity() })}
        >
          {t("rooms.leave")}
        </Button>
      </div>
      <div class="match-controls">
        <span>
          {t("friends.code")}{" "}
          <code data-testid="multiplayer-room-code">{room.code}</code>
        </span>
        <Button onClick={copy}>{t("friends.copy")}</Button>
        {share && <output>{t(`friends.${share}`)}</output>}
      </div>
      {host && setup && (
        <details>
          <summary>{t("rooms.saveSettings")}</summary>
          <RoomOptionsForm
            key={`${room.room_id}:${room.options.title}:${room.options.capacity}:${room.options.visibility}`}
            initial={room.options}
            label={t("rooms.saveSettings")}
            disabled={disabled}
            submit={(options) =>
              mutate({ type: "configure", ...identity(), options })
            }
          />
        </details>
      )}
    </Card>
  );
}
