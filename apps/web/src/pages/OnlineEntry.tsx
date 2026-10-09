import { useEffect, useMemo, useState } from "preact/hooks";
import type { ComponentType } from "preact";
import type { OnlineState } from "../services/online/controller";
import { roomCode } from "../services/online/invitation";
import { OnlineController } from "../services/online/controller";
import { RECONNECT_BUDGET_MS } from "../services/online/reconnect";
import { useUi } from "../ui/context";
import { useSnapshot } from "../ui/useSnapshot";
import { Button } from "../ui/atoms/Button";
import { Card } from "../ui/atoms/Card";
import { NavLink } from "../ui/molecules/NavLink";
import { Board } from "../ui/organisms/Board";
import { MatchSummary } from "../ui/organisms/MatchSummary";
import { OnlineResult } from "../ui/organisms/OnlineResult";
import { ReconnectNotice } from "../ui/organisms/ReconnectNotice";
import { MatchLayout } from "../ui/templates/MatchLayout";
export interface OnlinePanelProps {
  controller: OnlineController;
  state: OnlineState;
  locked: boolean;
  nickname: string;
}
type EntryProps = {
  mode: "queue" | "friends";
  Panel: ComponentType<OnlinePanelProps>;
};
export function OnlineEntry({ mode, Panel }: EntryProps) {
  const { t, services } = useUi(),
    auth = useSnapshot(services.auth);
  const candidate = services.auth.recoveryOwner();
  if (!services.auth.connected() && !candidate)
    return (
      <div class="reading-page">
        <Card>
          <h1>{t("status.title")}</h1>
          <p role={auth.status === "unavailable" ? "alert" : "status"}>
            {t(
              auth.status === "loading"
                ? "match.loading"
                : auth.status === "unavailable"
                  ? "queue.error"
                  : auth.account
                    ? "queue.nickname"
                    : "queue.signIn",
            )}
          </p>
          <div class="match-controls">
            <NavLink
              path={auth.account ? "/onboarding" : "/login"}
              class="button button--primary"
              query={
                mode === "friends"
                  ? {
                      return_path: "friends",
                      code: roomCode(
                        services.navigation
                          .current()
                          .searchParams.get("code") ?? "",
                      ),
                    }
                  : undefined
              }
            >
              {t(auth.account ? "auth.nicknameTitle" : "auth.title")}
            </NavLink>
            <NavLink path="/practice">{t("auth.local")}</NavLink>
            <NavLink path="/">{t("home")}</NavLink>
          </div>
        </Card>
      </div>
    );
  return (
    <OnlineSession
      mode={mode}
      Panel={Panel}
      key={`${auth.account?.id ?? candidate?.accountId}-${services.auth.revision()}`}
      nickname={auth.account?.nickname ?? t("match.you")}
    />
  );
}
function OnlineSession({
  nickname,
  mode,
  Panel,
}: EntryProps & { nickname: string }) {
  const { t, services } = useUi();
  const controller = useMemo(
    () => new OnlineController(services.auth, services.online),
    [services],
  );
  const state = useSnapshot(controller);
  const [locked, setLocked] = useState(false);
  useEffect(() => {
    let release: () => void;
    try {
      release = services.activity.hold();
    } catch {
      setLocked(true);
      return;
    }
    void controller.start();
    return () => {
      controller.dispose();
      release();
    };
  }, [controller, services]);
  const opponent =
    state.opponent === "bot"
      ? `${t("match.bot")}${state.difficulty ? ` · ${t(`match.${state.difficulty}`)}` : ""}`
      : t("queue.human");
  const error =
    state.error === "invalid_code"
      ? "friends.invalid"
      : state.error === "not_found"
        ? "friends.notFound"
        : state.error === "full"
          ? "friends.full"
          : state.error?.startsWith("input_")
            ? "match.rejected"
            : state.error === "disconnected" || state.error === "timeout"
              ? "queue.disconnected"
              : state.error === "capacity" || state.error === "rate_limited"
                ? "queue.limit"
                : state.error === "stale" || state.error === "busy"
                  ? "queue.stale"
                  : "queue.error";
  return (
    <>
      <div class="page-title">
        <div>
          <p class="eyebrow">{nickname}</p>
          <h1>{t(mode === "friends" ? "home.friend" : "queue.title")}</h1>
        </div>
        <NavLink path="/">{t("home")}</NavLink>
      </div>
      {(locked || state.error) && (
        <p role="alert">{t(locked ? "offline.error" : error)}</p>
      )}
      {state.status === "reconnecting" && (
        <ReconnectNotice
          kind="own"
          remainingMs={state.reconnectMs}
          maxMs={RECONNECT_BUDGET_MS}
        />
      )}
      {state.status === "playing" && state.view && !state.view.result && (
        <ReconnectNotice
          remainingMs={state.view.opponent.reconnect_ms}
          maxMs={state.view.rules.rules.reconnect_grace_ms}
        />
      )}
      {state.view?.result ? (
        <OnlineResult
          view={state.view}
          ownName={nickname}
          opponent={opponent}
          recording={state.recording}
          onAgain={() => void controller.start()}
          againLabel={mode === "friends" ? t("friends.again") : undefined}
        />
      ) : state.view ? (
        <MatchLayout
          summary={
            <>
              <MatchSummary
                view={state.view}
                opponent={opponent}
                ownName={nickname}
                heading={t("queue.online")}
              />
            </>
          }
        >
          <Board
            view={state.view}
            disabled={state.status !== "playing"}
            createRenderer={services.boardRenderer}
            onAction={(action) => controller.submit(action)}
          />
          <Button
            variant="primary"
            disabled={
              state.status !== "playing" ||
              state.view.phase !== "playing" ||
              state.view.own.stun_ms > 0 ||
              state.view.own.gauge < state.view.rules.rules.gauge_capacity
            }
            onClick={() => controller.submit({ type: "attack" })}
          >
            {t("home.attack")}
          </Button>
        </MatchLayout>
      ) : (
        <Panel
          controller={controller}
          state={state}
          locked={locked}
          nickname={nickname}
        />
      )}
      {state.status === "error" && !locked && (
        <div class="match-controls">
          <Button onClick={() => void controller.start()}>
            {t("queue.retry")}
          </Button>
          <NavLink path="/practice">{t("auth.local")}</NavLink>
        </div>
      )}
    </>
  );
}
