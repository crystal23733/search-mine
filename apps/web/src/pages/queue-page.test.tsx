import { fireEvent, render, screen, waitFor } from "@testing-library/preact";
import { expect, test, vi } from "vitest";
import { App } from "../App";
import { dailyTestPorts } from "../../test/daily-ports";
import { createAuth } from "../services/auth/session";
import { PROVIDERS, type AuthTransport } from "../services/auth/types";
import { createI18n } from "../services/i18n";
import { createNavigation } from "../services/navigation";
import { createPreferences } from "../services/preferences";
import { createLearning } from "../services/learning";
import { matchId, view } from "../../test/online-fixture";
import type { AppServices } from "../services/ports";
import type { OnlineEvent, LobbyResponse } from "@liar/protocol";
test("quick queue reuses the actual public board, keeps locale and activity through result, and disposes on home", async () => {
  const transport: AuthTransport = {
    bootstrap: async () => ({
      account: { id: matchId, nickname: "探偵" },
      session_revision: matchId,
      csrf: "proof",
      providers: PROVIDERS.map((p) => ({ provider: p.id, available: true })),
    }),
    identities: async () => [],
    start: async () => {
      throw Error();
    },
    nickname: async () => {
      throw Error();
    },
    export: async () => {
      throw Error();
    },
    erase: async () => {
      throw Error();
    },
    unlink: async () => {
      throw Error();
    },
    logout: async () => {},
  };
  const auth = createAuth(transport);
  await auth.refresh();
  const learning = createLearning();
  learning.mark("skipped");
  let event: (e: OnlineEvent) => void = () => {};
  const close = vi.fn(),
    connect = vi.fn(async () => ({
      listen: (e: (v: OnlineEvent) => void) => {
        event = e;
        event({
          v: 1,
          match_id: matchId,
          server_seq: 1,
          server_time_ms: 0,
          payload: {
            type: "snapshot",
            last_client_seq: 0,
            session_epoch: 1,
            view: { ...view(), phase: "playing" },
          },
        });
      },
      send: vi.fn(() => true),
      close,
    }));
  const services: AppServices = {
    ...dailyTestPorts(),
    auth,
    i18n: await createI18n("en"),
    navigation: createNavigation(window),
    preferences: createPreferences(),
    learning,
    practiceCore: async () => {
      throw Error();
    },
    trainingCore: async () => {
      throw Error();
    },
    boardRenderer: async () => {
      throw Error();
    },
    online: {
      lobby: vi.fn(async (_owner, command): Promise<LobbyResponse> => ({
        v: 1,
        server_time_ms: 0,
        state:
          command.type === "queue_join"
            ? {
                type: "matched",
                match_id: matchId,
                own_seat: 0,
                opponent: "human",
              }
            : { type: "idle" },
      })),
      connect,
    },
  };
  window.history.replaceState(null, "", "/en/queue");
  const mounted = render(<App services={services} />);
  fireEvent.click(
    await screen.findByRole("button", { name: "Find an opponent" }),
  );
  await waitFor(() =>
    expect(screen.getAllByRole("gridcell")).toHaveLength(256),
  );
  expect(services.activity.read().busy).toBe(true);
  expect(screen.getByText("Online match")).toBeTruthy();
  event({
    v: 1,
    match_id: matchId,
    server_seq: 2,
    server_time_ms: 0,
    payload: {
      type: "delta",
      view: {
        ...view(),
        phase: "playing",
        revision: 1,
        opponent: { ...view().opponent, reconnect_ms: 30000 },
      },
    },
  });
  await waitFor(() =>
    expect(screen.getByTestId("opponent-reconnect").textContent).toContain(
      "Opponent is reconnecting",
    ),
  );
  expect(
    screen
      .getByTestId("opponent-reconnect")
      .compareDocumentPosition(screen.getByRole("grid")) &
      Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy();
  expect(screen.queryByTestId("recording")).toBeNull();
  fireEvent.change(screen.getByRole("combobox", { name: "Language" }), {
    target: { value: "ko" },
  });
  await waitFor(() => expect(document.documentElement.lang).toBe("ko"));
  expect(connect).toHaveBeenCalledTimes(1);
  event({
    v: 1,
    match_id: matchId,
    server_seq: 3,
    server_time_ms: 0,
    payload: {
      type: "delta",
      view: {
        ...view(),
        phase: "playing",
        revision: 2,
        opponent: { ...view().opponent, reconnect_ms: 0 },
      },
    },
  });
  await waitFor(() =>
    expect(screen.getByTestId("opponent-reconnect").textContent).toContain(
      "서버 결과를 기다리고 있습니다",
    ),
  );
  expect(screen.queryByTestId("recording")).toBeNull();
  event({
    v: 1,
    match_id: matchId,
    server_seq: 4,
    server_time_ms: 0,
    payload: {
      type: "match_end",
      view: {
        ...view(),
        phase: "finished",
        revision: 3,
        result: { reason: "timeout", outcome: "draw", completed: true },
      },
      recording: "saved",
    },
  });
  await waitFor(() =>
    expect(screen.getByTestId("recording").textContent).toBe(
      "기록을 저장했습니다.",
    ),
  );
  expect(services.activity.read().busy).toBe(true);
  expect(screen.queryByRole("grid")).toBeNull();
  services.navigation.go("/");
  await waitFor(() => expect(services.activity.read().busy).toBe(false));
  expect(close).toHaveBeenCalled();
  mounted.unmount();
  auth.dispose();
});
