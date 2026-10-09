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
import type { AuthAccount } from "@liar/protocol";
import { AuthError } from "../services/auth/types";
async function setup(
  path: string,
  account: AuthAccount | null = TEST_ACCOUNT,
  enabled = true,
) {
  window.history.replaceState(null, "", path);
  const ports = authTestPorts(account, enabled);
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
test("login passes its invitation to the server transaction without browser storage", async () => {
  const ports = await setup(
    "/en/login?return_path=friends&code=ABCD2345",
    null,
  );
  fireEvent.click(
    await screen.findByRole("button", { name: "Continue with Google" }),
  );
  await waitFor(() =>
    expect(ports.authTransport.start).toHaveBeenCalledWith(
      "google",
      "login",
      { locale: "en", return_path: "friends", invite_code: "ABCD2345" },
      "fixture-memory-csrf",
      expect.any(AbortSignal),
    ),
  );
  expect(JSON.stringify({ ...localStorage, ...sessionStorage })).not.toContain(
    "ABCD2345",
  );
});

test("nickname onboarding uses the server result and skip preserves a validated invitation destination", async () => {
  const ports = await setup("/en/onboarding?return_path=friends&code=ABCD2345");
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
  expect(window.location.search).toContain("code=ABCD2345");
  expect(ports.services.learning.read()).toBe("skipped");
});

test("changing language before OAuth keeps the same invitation in the new server transaction", async () => {
  const ports = await setup(
    "/en/login?return_path=friends&code=ZZZZ6789",
    null,
  );
  fireEvent.change(screen.getByRole("combobox", { name: "Language" }), {
    target: { value: "ja" },
  });
  await waitFor(() => expect(document.documentElement.lang).toBe("ja"));
  fireEvent.click(
    screen.getByRole("button", {
      name: ports.services.i18n.t("ja", "auth.continue", {
        provider: "Google",
      }),
    }),
  );
  await waitFor(() =>
    expect(ports.authTransport.start).toHaveBeenCalledWith(
      "google",
      "login",
      { locale: "ja", return_path: "friends", invite_code: "ZZZZ6789" },
      "fixture-memory-csrf",
      expect.any(AbortSignal),
    ),
  );
  expect(window.location.search).toContain("code=ZZZZ6789");
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

test("unconfigured providers do not start a transaction and guest practice stays available", async () => {
  const ports = await setup("/en/login", null, false);
  expect(await screen.findAllByText("Not configured")).toHaveLength(4);
  for (const provider of ["Google", "Apple", "Kakao", "Naver"]) {
    const button = screen.getByRole("button", {
      name: `Continue with ${provider}`,
    }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    fireEvent.click(button);
  }
  expect(ports.authTransport.start).not.toHaveBeenCalled();
});

test("a first nickname keeps invalid input editable and only a server-accepted name starts the tutorial", async () => {
  const ports = await setup("/en/onboarding?return_path=https://evil.example", {
    ...TEST_ACCOUNT,
    nickname: null,
  });
  const input = (await screen.findByRole("textbox", {
    name: "Nickname",
  })) as HTMLInputElement;
  expect(input.value).toBe("");
  vi.mocked(ports.authTransport.nickname).mockRejectedValueOnce(
    new AuthError("auth_invalid"),
  );
  fireEvent.input(input, { target: { value: "X" } });
  fireEvent.click(
    screen.getByRole("button", { name: "Save and skip tutorial" }),
  );
  await screen.findByRole("alert");
  expect(input.value).toBe("X");
  expect(input.getAttribute("aria-invalid")).toBe("true");
  expect(window.location.pathname).toBe("/en/onboarding");
  fireEvent.click(screen.getByRole("button", { name: "QuietSolver" }));
  fireEvent.submit(input.closest("form")!);
  await waitFor(() => expect(window.location.pathname).toBe("/en/tutorial"));
  expect(new URLSearchParams(window.location.search).get("return")).toBe("/");
  expect(ports.auth.read().account?.nickname).toBe("QuietSolver");
});

test("settings downloads a minimal export, changes the canonical nickname and explicitly connects a provider", async () => {
  const ports = await setup("/en/settings");
  fireEvent.click(
    await screen.findByRole("button", { name: "Download my data" }),
  );
  await waitFor(() =>
    expect(ports.authEffects.download).toHaveBeenCalledWith({
      account: TEST_ACCOUNT,
      created_at: 1,
      last_seen_at: 2,
      identities: [{ provider: "google", linked_at: 1 }],
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Change nickname" }));
  const input = screen.getByRole("textbox", { name: "Nickname" });
  fireEvent.input(input, { target: { value: "e\u0301探偵" } });
  fireEvent.submit(input.closest("form")!);
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(screen.getByText("é探偵", { exact: true })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Connect Naver" }));
  await waitFor(() =>
    expect(ports.authTransport.start).toHaveBeenCalledWith(
      "naver",
      "link",
      { locale: "en", return_path: "settings" },
      "fixture-memory-csrf",
      expect.any(AbortSignal),
    ),
  );
});

test("reauthentication closes deletion confirmation and never repeats deletion automatically", async () => {
  const ports = await setup("/en/settings");
  vi.mocked(ports.authTransport.erase).mockRejectedValueOnce(
    new AuthError("reauth_required"),
  );
  fireEvent.click(
    await screen.findByRole("button", { name: "Delete account" }),
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Delete account permanently" }),
  );
  await screen.findByRole("heading", { name: "Confirm your sign-in again" });
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(ports.authTransport.start).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Confirm with Google" }));
  await waitFor(() =>
    expect(ports.authTransport.start).toHaveBeenCalledWith(
      "google",
      "reauth",
      { locale: "en", return_path: "settings" },
      "fixture-memory-csrf",
      expect.any(AbortSignal),
    ),
  );
  expect(ports.authTransport.erase).toHaveBeenCalledTimes(1);
});

test("an explicit deletion ends account authority and presents manual Apple disconnection when returned by the server", async () => {
  const ports = await setup("/en/settings");
  const original = vi
    .mocked(ports.authTransport.erase)
    .getMockImplementation()!;
  vi.mocked(ports.authTransport.erase).mockImplementationOnce(
    async (...args) => {
      await original(...args);
      return { manual_apple_disconnect: true };
    },
  );
  fireEvent.click(
    await screen.findByRole("button", { name: "Delete account" }),
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Delete account permanently" }),
  );
  await screen.findByText("Your Liar Sweeper account was deleted.");
  expect(screen.getByText(/Open your Apple account settings/)).toBeTruthy();
  expect(ports.auth.account()).toBeNull();
  expect(ports.auth.connected()).toBe(false);
});
