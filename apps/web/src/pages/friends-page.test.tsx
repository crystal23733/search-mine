import { fireEvent, render, screen, waitFor } from "@testing-library/preact";
import { beforeAll, expect, test, vi } from "vitest";
import type { AuthAccount, LobbyResponse } from "@liar/protocol";
import { App } from "../App";
import { dailyTestPorts } from "../../test/daily-ports";
import { createAuth } from "../services/auth/session";
import { PROVIDERS, type AuthTransport } from "../services/auth/types";
import { createI18n } from "../services/i18n";
import { createNavigation } from "../services/navigation";
import { createPreferences } from "../services/preferences";
import { createLearning } from "../services/learning";
import { matchId } from "../../test/online-fixture";
import type { AppServices } from "../services/ports";
// Cold route loading is exercised by the production browser tests.
beforeAll(async () => {
  await import("./FriendsPage");
});
async function setup(account: AuthAccount | null) {
  const transport: AuthTransport = {
    bootstrap: async () => ({
      account,
      session_revision: account ? matchId : null,
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
  let state: LobbyResponse["state"] = { type: "idle" };
  const lobby = vi.fn(
    async (_owner, command, _signal: AbortSignal): Promise<LobbyResponse> => {
      if (command.type === "room_create" || command.type === "room_join")
        state = {
          type: "room",
          room_id: matchId,
          code: "ABCD2345",
          own_seat: 1,
          occupied: [true, true],
          ready: [false, false],
          expires_at_ms: 600000,
        };
      if (command.type === "ready" && state.type === "room")
        state = { ...state, ready: [false, command.ready] };
      if (command.type === "cancel") state = { type: "idle" };
      return { v: 1, server_time_ms: 0, state };
    },
  );
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
      lobby,
      connect: vi.fn(async () => {
        throw Error();
      }),
    },
  };
  return { services, auth, lobby };
}
test("friend invitation is explicit, shares only the public locale/code, preserves controller across locale and leaves exact room", async () => {
  window.history.replaceState(
    null,
    "",
    "/en/friends?code=abcd2345&token=do-not-share",
  );
  const s = await setup({ id: matchId, nickname: "Detective" });
  s.services.share.copy = vi.fn(async () => {});
  s.services.share.nativeShare = vi.fn(async () => {
    throw Error();
  });
  const mounted = render(<App services={s.services} />);
  const input = await screen.findByRole("textbox", { name: "Room code" });
  expect((input as HTMLInputElement).value).toBe("ABCD2345");
  expect(s.lobby).toHaveBeenCalledTimes(1);
  fireEvent.click(
    screen.getByRole("button", { name: "Join room", exact: true }),
  );
  await screen.findByTestId("room-code");
  expect(s.services.activity.read().busy).toBe(true);
  expect(s.services.online.connect).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Ready", exact: true }));
  await waitFor(() =>
    expect(
      screen
        .getByRole("button", { name: "Not ready", exact: true })
        .getAttribute("aria-pressed"),
    ).toBe("true"),
  );
  fireEvent.click(screen.getByRole("button", { name: "Copy invitation link" }));
  const readySignal = s.lobby.mock.calls.at(-1)![2];
  await waitFor(() =>
    expect(s.services.share.copy).toHaveBeenCalledWith(
      `${window.location.origin}/en/friends?code=ABCD2345`,
    ),
  );
  fireEvent.click(screen.getByRole("button", { name: "Share invitation" }));
  await screen.findByText(
    "Sharing was canceled or unavailable. Copy the link instead.",
  );
  fireEvent.change(screen.getByRole("combobox", { name: "Language" }), {
    target: { value: "ko" },
  });
  await waitFor(() => expect(document.documentElement.lang).toBe("ko"));
  expect(readySignal.aborted).toBe(false);
  expect(
    s.lobby.mock.calls
      .filter(([, command]) => command.type !== "status")
      .map(([, command]) => command),
  ).toEqual([
    { type: "room_join", code: "ABCD2345" },
    { type: "ready", room_id: matchId, ready: true },
  ]);
  fireEvent.click(screen.getByRole("button", { name: "방 나가기" }));
  await screen.findByRole("button", { name: "방 만들기", exact: true });
  expect(s.lobby).toHaveBeenLastCalledWith(
    expect.anything(),
    { type: "cancel", identity: { kind: "room", room_id: matchId } },
    expect.any(AbortSignal),
  );
  s.services.navigation.go("/");
  await waitFor(() => expect(s.services.activity.read().busy).toBe(false));
  mounted.unmount();
  s.auth.dispose();
});
test("guest friend invitation carries the canonical code to login without creating a lobby account or joining", async () => {
  window.history.replaceState(null, "", "/en/friends?code=abcd2345");
  const s = await setup(null);
  const mounted = render(<App services={s.services} />);
  fireEvent.click(
    await screen.findByRole("link", {
      name: "Use an account you already have",
      exact: true,
    }),
  );
  expect(s.services.navigation.current().pathname).toBe("/en/login");
  expect(s.services.navigation.current().searchParams.get("return_path")).toBe(
    "friends",
  );
  expect(s.services.navigation.current().searchParams.get("code")).toBe(
    "ABCD2345",
  );
  expect(s.lobby).not.toHaveBeenCalled();
  expect(s.services.activity.read().busy).toBe(false);
  mounted.unmount();
  s.auth.dispose();
});
test("an existing server room overrides a different invitation and a late failed copy cannot replace the next room's success", async () => {
  window.history.replaceState(null, "", "/en/friends?code=ABCD2345");
  const s = await setup({ id: matchId, nickname: "Detective" });
  const oldRoom: LobbyResponse = {
    v: 1,
    server_time_ms: 0,
    state: {
      type: "room",
      room_id: matchId,
      code: "ZZZZ6789",
      own_seat: 1,
      occupied: [true, true],
      ready: [true, false],
      expires_at_ms: 600000,
    },
  };
  const nextRoom: LobbyResponse = {
    ...oldRoom,
    state: {
      ...(oldRoom.state as Extract<LobbyResponse["state"], { type: "room" }>),
      room_id: "22222222-2222-4222-8222-222222222222",
      code: "EFGH6789",
      own_seat: 0,
      occupied: [true, false],
      ready: [false, false],
    },
  };
  s.lobby.mockResolvedValue(oldRoom);
  let rejectOld!: (error: Error) => void;
  s.services.share.copy = vi
    .fn()
    .mockImplementationOnce(
      () =>
        new Promise<void>((_resolve, reject) => {
          rejectOld = reject;
        }),
    )
    .mockResolvedValue(undefined);
  const mounted = render(<App services={s.services} />);
  await waitFor(() =>
    expect(screen.getByTestId("room-code").textContent).toBe("ZZZZ6789"),
  );
  expect(s.lobby).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("textbox", { name: "Room code" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Copy invitation link" }));
  s.lobby.mockResolvedValueOnce({
    v: 1,
    server_time_ms: 0,
    state: { type: "idle" },
  });
  fireEvent.click(
    screen.getByRole("button", { name: "Leave room", exact: true }),
  );
  await screen.findByRole("button", { name: "Create room", exact: true });
  s.lobby.mockResolvedValue(nextRoom);
  fireEvent.click(
    screen.getByRole("button", { name: "Create room", exact: true }),
  );
  await waitFor(() =>
    expect(screen.getByTestId("room-code").textContent).toBe("EFGH6789"),
  );
  fireEvent.click(screen.getByRole("button", { name: "Copy invitation link" }));
  await screen.findByText("Invitation link copied.");
  rejectOld(Error("old_copy_failed"));
  await Promise.resolve();
  await Promise.resolve();
  expect(screen.getByText("Invitation link copied.")).toBeTruthy();
  expect(
    screen.queryByText(
      "The link could not be copied. Select the room code and send it to your friend.",
    ),
  ).toBeNull();
  s.auth.invalidate();
  await screen.findByRole("link", {
    name: "Use an account you already have",
    exact: true,
  });
  expect(screen.queryByTestId("room-code")).toBeNull();
  await waitFor(() => expect(s.services.activity.read().busy).toBe(false));
  mounted.unmount();
  s.auth.dispose();
});
