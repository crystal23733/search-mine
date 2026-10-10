import { fireEvent, render, screen, waitFor } from "@testing-library/preact";
import { beforeAll, expect, test, vi } from "vitest";
import { App } from "../App";
import { dailyTestPorts } from "../../test/daily-ports";
import { createAuth } from "../services/auth/session";
import { PROVIDERS, type AuthTransport } from "../services/auth/types";
import { createI18n } from "../services/i18n";
import { createNavigation } from "../services/navigation";
import { createPreferences } from "../services/preferences";
import { createLearning } from "../services/learning";
import { matchId, view, personalResult } from "../../test/online-fixture";
import type { AppServices } from "../services/ports";
import type { OnlineEvent, LobbyResponse } from "@liar/protocol";
// Cold route loading is exercised by the production browser tests.
beforeAll(async () => {
  await import("./QueuePage");
});
async function queueScenario() {
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
            session_epoch: connect.mock.calls.length,
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
      latestResult: vi.fn(),
      result: vi.fn(async () => {
        throw Error("unavailable");
      }),
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
  const dispose = () => {
    mounted.unmount();
    auth.dispose();
  };
  try {
    fireEvent.click(
      await screen.findByRole("button", { name: "Find an opponent" }),
    );
    await waitFor(() =>
      expect(screen.getAllByRole("gridcell")).toHaveLength(256),
    );
  } catch (error) {
    dispose();
    throw error;
  }
  return {
    services,
    auth,
    connect,
    close,
    dispose,
    emit: (message: OnlineEvent) => event(message),
  };
}
test("quick queue displays all public cells, holds activity until home and disposes its connection", async () => {
  const { services, close, dispose } = await queueScenario();
  try {
    expect(services.activity.read().busy).toBe(true);
    expect(screen.getByText("Online match")).toBeTruthy();
    services.navigation.go("/");
    await waitFor(() => expect(services.activity.read().busy).toBe(false));
    expect(close).toHaveBeenCalled();
  } finally {
    dispose();
  }
});
test("explicit stored result replaces the live result with personal statistics, keeps locale and leaves safely", async () => {
  const s = await queueScenario();
  try {
    vi.mocked(s.services.online.result).mockResolvedValue(personalResult());
    s.emit({
      v: 1,
      match_id: matchId,
      server_seq: 2,
      server_time_ms: 0,
      payload: {
        type: "match_end",
        view: {
          ...view(),
          phase: "finished",
          revision: 1,
          result: { reason: "timeout", outcome: "win", completed: true },
        },
        recording: "pending",
      },
    });
    fireEvent.click(
      await screen.findByRole("button", { name: "Check stored result" }),
    );
    await screen.findByTestId("personal-result");
    expect(screen.queryByRole("grid")).toBeNull();
    expect(screen.queryByRole("progressbar")).toBeNull();
    expect(screen.queryByText("Opponent")).toBeNull();
    expect(screen.getByText("Safe cells opened: 8")).toBeTruthy();
    expect(screen.getByText("Mistakes: 2")).toBeTruthy();
    expect(screen.getByTestId("recording").textContent).toBe("Result saved.");
    fireEvent.change(screen.getByRole("combobox", { name: "Language" }), {
      target: { value: "ko" },
    });
    await waitFor(() =>
      expect(screen.getByTestId("personal-result").textContent).toContain(
        "저장된 내 결과",
      ),
    );
    expect(s.services.online.result).toHaveBeenCalledTimes(1);
    expect(s.services.activity.read().busy).toBe(true);
    s.services.navigation.go("/");
    await waitFor(() => expect(s.services.activity.read().busy).toBe(false));
  } finally {
    s.dispose();
  }
});
test("an unknown abort shows no fabricated statistics and retains safe exit", async () => {
  const s = await queueScenario();
  try {
    vi.mocked(s.services.online.result).mockResolvedValue({
      ...personalResult(),
      own: null,
      end_elapsed_ms: null,
      result: { reason: "server_failure", outcome: "abort", completed: false },
    });
    s.emit({
      v: 1,
      match_id: matchId,
      server_seq: 2,
      server_time_ms: 0,
      payload: {
        type: "match_end",
        view: {
          ...view(),
          phase: "finished",
          revision: 1,
          result: { reason: "timeout", outcome: "win", completed: true },
        },
        recording: "pending",
      },
    });
    fireEvent.click(
      await screen.findByRole("button", { name: "Check stored result" }),
    );
    const result = await screen.findByTestId("personal-result");
    expect(result.textContent).toContain(
      "Statistics for this match are unavailable.",
    );
    expect(result.textContent).not.toMatch(/Safe cells opened:|Mistakes:|\d/);
    expect(screen.queryByRole("grid")).toBeNull();
    expect(screen.queryByRole("progressbar")).toBeNull();
    expect(screen.getByTestId("recording").textContent).toBe("Result saved.");
    expect(s.services.activity.read().busy).toBe(true);
    fireEvent.click(
      screen.getByRole("button", { name: "Find a new opponent" }),
    );
    await waitFor(() =>
      expect(screen.queryByTestId("personal-result")).toBeNull(),
    );
  } finally {
    s.dispose();
  }
});
test("suspended queue session keeps all public cells read-only until fresh authenticated recovery", async () => {
  const { services, auth, connect, dispose } = await queueScenario();
  try {
    auth.suspend();
    await waitFor(() => {
      expect(screen.getAllByRole("gridcell")).toHaveLength(256);
      expect(screen.getByTestId("own-reconnect").textContent).toContain(
        "Trying to reconnect",
      );
    });
    expect(services.activity.read().busy).toBe(true);
    expect(
      screen.getByRole<HTMLButtonElement>("button", {
        name: "Attack",
        exact: true,
      }).disabled,
    ).toBe(true);
    expect(auth.connected()).toBe(false);
    expect(connect).toHaveBeenCalledTimes(1);
    await auth.resume({ accountId: matchId, revision: auth.revision() });
    await waitFor(() => expect(connect).toHaveBeenCalledTimes(2));
    await waitFor(() =>
      expect(screen.queryByTestId("own-reconnect")).toBeNull(),
    );
    expect(services.activity.read().busy).toBe(true);
  } finally {
    dispose();
  }
});
test("recovered queue keeps opponent grace through locale and holds activity until the saved server result closes", async () => {
  const { services, auth, connect, close, dispose, emit } =
    await queueScenario();
  try {
    auth.suspend();
    await screen.findByTestId("own-reconnect");
    await auth.resume({ accountId: matchId, revision: auth.revision() });
    await waitFor(() => expect(connect).toHaveBeenCalledTimes(2));
    await waitFor(() =>
      expect(screen.queryByTestId("own-reconnect")).toBeNull(),
    );
    emit({
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
    expect(connect).toHaveBeenCalledTimes(2);
    emit({
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
    emit({
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
  } finally {
    dispose();
  }
});
