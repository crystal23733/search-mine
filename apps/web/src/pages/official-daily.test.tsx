import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/preact";
import { expect, test, vi } from "vitest";
import { dailyTestPorts } from "../../test/daily-ports";
import { officialView, deferred } from "../../test/official-daily";
import { createI18n } from "../services/i18n";
import { createNavigation } from "../services/navigation";
import { createPreferences } from "../services/preferences";
import { createLearning } from "../services/learning";
import { Ui } from "../ui/context";
import { OfficialDailyPage } from "./OfficialDailyPage";
import type { AppServices } from "../services/ports";
import type { AuthState } from "../services/auth/types";
import type { DailyChallengeApi } from "../services/daily/official";
async function setup(guest = false) {
  const base = dailyTestPorts(),
    view = officialView();
  let authState: AuthState = {
    ...base.auth.read(),
    status: "ready",
    account: guest ? null : { id: "account-a", nickname: "PrivateName" },
  };
  let revision = 1;
  const listeners = new Set<() => void>();
  const start = vi.fn<DailyChallengeApi["start"]>(async (owner, attempt) => ({
    v: 1,
    account_id: owner.accountId,
    attempt_id: attempt,
    metadata: view.metadata,
    rules: view.rules,
    ticket_id: "ticket",
    ttl_ms: 300000,
    heartbeat_interval_ms: 15000,
  }));
  const receipt = {
    completion_id: "private-completion",
    attempt_id: "first-attempt",
    metadata: view.metadata,
    opened_safe: 216,
    mistakes: 3,
    replayed_elapsed_ms: 5000,
    verified_elapsed_ms: 12345,
    completed_at: 1791331200,
  };
  const own = vi.fn<DailyChallengeApi["own"]>(async (owner) => ({
    v: 1,
    account_id: owner.accountId,
    receipt,
    trust: "verified_time",
  }));
  const core = {
    init: vi.fn(async () => view),
    snapshot: vi.fn(),
    advance: vi.fn(async () => view),
    step: vi.fn(),
    replay: vi.fn(),
    dispose: vi.fn(),
  };
  const copy = vi.fn<AppServices["share"]["copy"]>(async () => {}),
    download = vi.fn<AppServices["share"]["download"]>(async () => {}),
    nativeShare = vi.fn<AppServices["share"]["nativeShare"]>(async () => {});
  const services: AppServices = {
    ...base,
    auth: {
      ...base.auth,
      read: () => authState,
      account: () => authState.account?.id ?? null,
      revision: () => revision,
      connected: () => true,
      subscribe: (cb) => {
        listeners.add(cb);
        return () => {
          listeners.delete(cb);
        };
      },
    },
    dailyChallenge: {
      ...base.dailyChallenge,
      start,
      own,
      definition: async () => ({
        v: 1,
        metadata: view.metadata,
        rules: view.rules,
      }),
    },
    dailyCore: async () => core,
    i18n: await createI18n("en"),
    navigation: createNavigation(window),
    preferences: createPreferences(),
    learning: createLearning(),
    practiceCore: vi.fn(),
    trainingCore: vi.fn(),
    boardRenderer: async () => {
      throw Error("DOM fallback");
    },
    share: { copy, download, nativeShare },
  };
  const rendered = render(
    <Ui.Provider
      value={{
        locale: "en",
        services,
        preferences: services.preferences.read(),
        selectLocale: () => {},
        t: (key, data) => services.i18n.t("en", key, data),
      }}
    >
      <OfficialDailyPage />
    </Ui.Provider>,
  );
  const change = () => {
    authState = {
      ...authState,
      account: { id: "account-b", nickname: "Other" },
    };
    revision++;
    for (const cb of listeners) cb();
  };
  return {
    ...rendered,
    services,
    start,
    own,
    core,
    copy,
    download,
    nativeShare,
    change,
    receipt,
  };
}
test("guest can reach local practice and login; official starts require a current account confirmation", async () => {
  const guest = await setup(true);
  expect(
    (
      screen.getByRole("button", {
        name: "Start official challenge",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  expect(
    screen
      .getByRole("link", { name: "Sign in for official play" })
      .getAttribute("href"),
  ).toContain("return=daily");
  guest.unmount();
  const s = await setup();
  fireEvent.click(
    screen.getByRole("button", { name: "Start official challenge" }),
  );
  const dialog = await screen.findByRole("dialog");
  expect(dialog.textContent).toContain("PrivateName");
  expect(s.start).not.toHaveBeenCalled();
  await act(async () => s.change());
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(s.start).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Start official challenge" }),
  );
  fireEvent.click(
    within(screen.getByRole("dialog")).getByRole("button", {
      name: "Confirm and start",
    }),
  );
  await screen.findByRole("grid");
  expect(s.start.mock.calls[0][0].accountId).toBe("account-b");
  s.unmount();
  expect(s.core.dispose).toHaveBeenCalledTimes(1);
  expect(s.services.activity.read().busy).toBe(false);
});
test("own result and all share actions use the receipt without daily writes and disappear on account replacement", async () => {
  const s = await setup();
  fireEvent.click(
    screen.getByRole("button", { name: "Check my daily record" }),
  );
  await screen.findByRole("heading", { name: "Your verified daily record" });
  expect(screen.getByLabelText("Server time").textContent).toBe("0:12.345");
  fireEvent.click(screen.getByRole("button", { name: "Copy result" }));
  await waitFor(() => expect(s.copy).toHaveBeenCalled());
  expect(s.copy.mock.calls[0][0]).not.toMatch(
    /PrivateName|private-completion|first-attempt|seed|ticket/,
  );
  fireEvent.click(screen.getByRole("button", { name: "Download PNG" }));
  await waitFor(() => expect(s.download).toHaveBeenCalled());
  fireEvent.click(
    screen.getByRole("button", { name: "Share result", exact: true }),
  );
  await waitFor(() => expect(s.nativeShare).toHaveBeenCalled());
  expect(s.start).not.toHaveBeenCalled();
  await act(async () => s.change());
  expect(
    screen.queryByRole("heading", { name: "Your verified daily record" }),
  ).toBeNull();
  s.unmount();
});
test("own failures are retryable, no-time is explicit, and layout cleanup aborts an outstanding read", async () => {
  const s = await setup();
  s.own.mockRejectedValueOnce(Error("503"));
  const check = screen.getByRole("button", { name: "Check my daily record" });
  fireEvent.click(check);
  await screen.findByRole("alert");
  s.own.mockResolvedValueOnce({
    v: 1,
    account_id: "account-a",
    receipt: { ...s.receipt, verified_elapsed_ms: null },
    trust: "verified_completion",
  });
  fireEvent.click(check);
  await screen.findByText("No verified server time is available.");
  s.copy.mockRejectedValueOnce(Error("clipboard"));
  fireEvent.click(screen.getByRole("button", { name: "Copy result" }));
  await screen.findByText(s.services.i18n.t("en", "daily.shareError"));
  const gate = deferred<Awaited<ReturnType<DailyChallengeApi["own"]>>>();
  s.own.mockReturnValueOnce(gate.promise);
  fireEvent.click(check);
  await waitFor(() => expect(s.own).toHaveBeenCalledTimes(3));
  const signal = s.own.mock.calls[2][2]!;
  s.unmount();
  expect(signal.aborted).toBe(true);
  await act(async () =>
    gate.resolve({
      v: 1,
      account_id: "account-a",
      receipt: null,
      trust: "verified_completion",
    }),
  );
});
test("a verified completion replaces the running timing hint beside the finished board", async () => {
  const s = await setup(),
    view = officialView();
  s.core.step.mockResolvedValueOnce({
    view: { ...view, phase: "finished", completed: true, elapsed_ms: 5000 },
    ack: {
      command_id: 1,
      revision: 1,
      status: "applied",
      error: null,
      duplicate: false,
    },
  });
  s.core.replay.mockResolvedValueOnce({
    v: 1,
    metadata: view.metadata,
    inputs: [],
    final_time_ms: 8000,
  });
  s.services.dailyChallenge.submitTimed = async (owner, attempt) => ({
    v: 1,
    account_id: owner.accountId,
    attempt_id: attempt,
    disposition: "official",
    trust: "verified_time",
    receipt: { ...s.receipt, attempt_id: attempt },
  });
  fireEvent.click(
    screen.getByRole("button", { name: "Start official challenge" }),
  );
  fireEvent.click(
    within(await screen.findByRole("dialog")).getByRole("button", {
      name: "Confirm and start",
    }),
  );
  await screen.findByRole("grid");
  fireEvent.keyDown(screen.getAllByRole("gridcell")[1], { key: "Enter" });
  await screen.findByRole("heading", { name: "Your verified daily record" });
  expect(
    screen.queryByText(
      "Server timing is being checked. This running clock is a personal reference.",
    ),
  ).toBeNull();
  expect(
    screen.getAllByText("Completion and server time verified"),
  ).toHaveLength(2);
  s.unmount();
});
