import { fireEvent, render, screen, waitFor } from "@testing-library/preact";
import { expect, test, vi } from "vitest";
import { App } from "../App";
import { authTestPorts, TEST_ACCOUNT } from "../../test/auth-ports";
import { dailyTestPorts } from "../../test/daily-ports";
import { overview, history, ME_NOW } from "../../test/me-data";
import { createI18n } from "../services/i18n";
import { createNavigation } from "../services/navigation";
import { createPreferences } from "../services/preferences";
import { createLearning } from "../services/learning";
import type { AppServices } from "../services/ports";
import type { AuthAccount } from "@liar/protocol";
import { MeFailure } from "../services/me/types";
async function setup(
  path = "/en/me",
  account: AuthAccount | null = TEST_ACCOUNT,
) {
  window.history.replaceState(null, "", path);
  const auth = authTestPorts(account, true);
  const me = {
    overview: vi.fn().mockResolvedValue({
      ...overview(),
      account: { ...overview().account, nickname: account?.nickname ?? null },
    }),
    history: vi.fn().mockImplementation(async (_owner, selection) =>
      selection.opponent === "bot"
        ? {
            ...history(),
            opponent: "bot",
            total: 0,
            items: [],
            online: {
              wins: 0,
              losses: 0,
              draws: 0,
              aborted: 0,
              cancelled: 0,
              played: 0,
              win_rate: null,
            },
          }
        : history(),
    ),
  };
  const services: AppServices = {
    ...dailyTestPorts(),
    ...auth,
    me,
    i18n: await createI18n("en"),
    navigation: createNavigation(window),
    preferences: createPreferences(),
    learning: createLearning(),
    wallClock: () => ME_NOW * 1000,
    practiceCore: async () => {
      throw Error();
    },
    trainingCore: async () => {
      throw Error();
    },
    boardRenderer: async () => {
      throw Error();
    },
  };
  render(<App services={services} />);
  return { ...auth, services, me };
}
test("my info displays server summaries, navigates private tabs and keeps settings on the same device page", async () => {
  const f = await setup();
  await screen.findByRole("heading", { name: "My info", level: 1 });
  await screen.findByTestId("me-summary");
  expect(screen.getByTestId("me-win-rate").textContent).toBe("100%");
  expect(
    document.querySelector('meta[name="robots"]')?.getAttribute("content"),
  ).toContain("noindex");
  fireEvent.click(
    screen.getByRole("link", { name: "Match history", exact: true }),
  );
  await screen.findByRole("heading", { name: "Match history", level: 1 });
  await screen.findByTestId("me-history-list");
  fireEvent.change(screen.getByRole("combobox", { name: "Opponent" }), {
    target: { value: "bot" },
  });
  await screen.findByText("No stored records in this view.");
  expect(screen.getByTestId("me-win-rate").textContent).toBe("—");
  const settings = screen
    .getAllByRole("link", { name: "Settings", exact: true })
    .find((link) => link.getAttribute("href") === "/en/settings")!;
  fireEvent.click(settings);
  await screen.findByRole("heading", { name: "Settings", level: 1 });
  expect(document.querySelector('meta[name="robots"]')).toBeNull();
  expect(f.me.history).toHaveBeenCalled();
});
test("guest information links to sign-in and device settings without creating a server account", async () => {
  const f = await setup("/en/me", null);
  await screen.findByText("Sign in to view your information.");
  expect(
    screen
      .getByRole("link", { name: "Sign in", exact: true })
      .getAttribute("href"),
  ).toBe("/en/login?return_path=me");
  expect(f.auth.account()).toBeNull();
  expect(f.me.overview).not.toHaveBeenCalled();
});
test("an unset nickname still permits account management and no download action exists", async () => {
  await setup("/en/me/account", { ...TEST_ACCOUNT, nickname: null });
  await screen.findByRole("button", { name: "Delete account", exact: true });
  expect(
    screen
      .getByRole("button", { name: "Change nickname" })
      .hasAttribute("disabled"),
  ).toBe(false);
  expect(
    screen.queryByRole("button", { name: /download my data/i }),
  ).toBeNull();
});
test("private data disappears on offline authority loss and failed reads remain retryable", async () => {
  const f = await setup();
  await screen.findByTestId("me-summary");
  f.auth.suspend();
  await waitFor(() => expect(screen.queryByTestId("me-summary")).toBeNull());
  expect(screen.queryByText("Player", { exact: true })).toBeNull();
  f.me.overview.mockRejectedValueOnce(new MeFailure());
  await f.auth.refresh();
  await screen.findByRole("alert");
  expect(screen.queryByTestId("me-summary")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Try again" }));
  await screen.findByTestId("me-summary");
});
test("privacy distinguishes the server view from unavailable CMP setup and retains shared device controls", async () => {
  await setup("/en/me/privacy");
  const server = await screen.findByTestId("me-server-consent");
  expect(server.textContent).toContain(
    "Consent settings are not available yet.",
  );
  expect(
    screen
      .getByRole("button", { name: "Reopen consent choices" })
      .hasAttribute("disabled"),
  ).toBe(true);
});
