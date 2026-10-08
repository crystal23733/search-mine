import { fireEvent, render, screen, waitFor } from "@testing-library/preact";
import { expect, test, vi } from "vitest";
import { App } from "../App";
import { dailyTestPorts } from "../../test/daily-ports";
import { authTestPorts, TEST_ACCOUNT } from "../../test/auth-ports";
import { createI18n } from "../services/i18n";
import { createNavigation } from "../services/navigation";
import { createPreferences } from "../services/preferences";
import { createLearning } from "../services/learning";
import type { AppServices } from "../services/ports";
async function setup(
  path: string,
  account: typeof TEST_ACCOUNT | null = TEST_ACCOUNT,
) {
  window.history.replaceState(null, "", path);
  const ports = authTestPorts(account, true);
  await ports.auth.refresh();
  const services: AppServices = {
    ...dailyTestPorts(),
    ...ports,
    i18n: await createI18n("en"),
    navigation: createNavigation(window),
    preferences: createPreferences(),
    learning: createLearning(),
    practiceCore: async () => {
      throw Error("unavailable");
    },
    trainingCore: async () => {
      throw Error("unavailable");
    },
    boardRenderer: async () => {
      throw Error("unavailable");
    },
  };
  render(<App services={services} />);
  return { ...ports, services };
}
test("login offers the four minimal providers, disabled setup and guest practice", async () => {
  const ports = await setup("/en/login", null);
  expect(
    await screen.findByRole("heading", {
      name: "Use an account you already have",
    }),
  ).toBeTruthy();
  for (const provider of ["Google", "Apple", "Kakao", "Naver"])
    expect(
      screen.getByRole("button", { name: `Continue with ${provider}` }),
    ).toBeTruthy();
  expect(screen.queryByLabelText(/password|email|phone/i)).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Continue with Google" }));
  await waitFor(() =>
    expect(ports.authEffects.redirect).toHaveBeenCalledWith(
      "https://accounts.google.com/o/oauth2/v2/auth?state=fixture",
    ),
  );
  expect(
    screen
      .getByRole("link", { name: "Practice without signing in" })
      .getAttribute("href"),
  ).toBe("/en/practice");
});
test("nickname onboarding uses the server result and skip preserves a validated invitation destination", async () => {
  const ports = await setup("/en/onboarding?return_path=friends&code=ABCD1234");
  // A nickname form is also used by the already authenticated account.
  const input = await screen.findByRole("textbox", { name: "Nickname" });
  fireEvent.input(input, { target: { value: "e\u0301探偵" } });
  fireEvent.click(
    screen.getByRole("button", { name: "Save and skip tutorial" }),
  );
  await waitFor(() =>
    expect(ports.auth.read().account?.nickname).toBe("é探偵"),
  );
  await waitFor(() => expect(window.location.pathname).toBe("/en/friends"));
  expect(window.location.search).toContain("code=ABCD1234");
  expect(ports.services.learning.read()).toBe("skipped");
});
test("a blocked provider redirect reports a recoverable error", async () => {
  const ports = await setup("/en/login", null);
  vi.mocked(ports.authEffects.redirect).mockImplementation(() => {
    throw Error("navigation blocked");
  });
  fireEvent.click(
    await screen.findByRole("button", { name: "Continue with Google" }),
  );
  expect((await screen.findByRole("alert")).textContent).toContain("Retry");
  expect(ports.auth.read().account).toBeNull();
});
test("settings protect the last provider and an open deletion confirmation keeps its original owner", async () => {
  const ports = await setup("/en/settings");
  expect(
    (
      (await screen.findByRole("button", {
        name: "Disconnect Google",
      })) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Delete account" }));
  expect(screen.getByRole("dialog")).toBeTruthy();
  const other = {
    id: "a0000000-0000-4000-8000-000000000002",
    nickname: "Other",
  };
  vi.mocked(ports.authTransport.bootstrap).mockResolvedValue({
    account: other,
    session_revision: "b0000000-0000-4000-8000-000000000002",
    csrf: "new",
    providers: [
      { provider: "google", available: true },
      { provider: "apple", available: true },
      { provider: "kakao", available: true },
      { provider: "naver", available: true },
    ],
  });
  await ports.auth.refresh();
  await waitFor(() =>
    expect(
      (
        screen.getByRole("button", {
          name: "Delete account permanently",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true),
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Delete account permanently" }),
  );
  expect(ports.authTransport.erase).not.toHaveBeenCalled();
});
