import { render, screen, fireEvent, waitFor } from "@testing-library/preact";
import { expect, test, vi } from "vitest";
import { App } from "../App";
import { dailyTestPorts } from "../../test/daily-ports";
import { authTestPorts, TEST_ACCOUNT } from "../../test/auth-ports";
import { personalResult } from "../../test/online-fixture";
import { createI18n } from "../services/i18n";
import { createNavigation } from "../services/navigation";
import { createPreferences } from "../services/preferences";
import { createLearning } from "../services/learning";
import type { AppServices } from "../services/ports";
import type { PersonalResult } from "@liar/protocol";
async function setup(guest = false, path = "/en/") {
  window.history.replaceState(null, "", path);
  const ports = dailyTestPorts(),
    account = authTestPorts(guest ? null : TEST_ACCOUNT, true);
  const latestResult = vi.fn(async (): Promise<PersonalResult | null> =>
    personalResult(),
  );
  const services: AppServices = {
    ...ports,
    ...account,
    online: { ...ports.online, latestResult },
    i18n: await createI18n("en"),
    navigation: createNavigation(window),
    preferences: createPreferences(),
    learning: createLearning(),
    trainingCore: vi.fn(),
    practiceCore: vi.fn(),
    boardRenderer: vi.fn(),
  };
  services.learning.mark("skipped");
  const view = render(<App services={services} />);
  return { services, latestResult, view };
}
test("home offers exactly one authenticated link and a fixed ID-free result page reuses the personal result", async () => {
  const s = await setup();
  const link = await screen.findByRole("link", { name: "Latest match result" });
  expect(
    screen.getAllByRole("link", { name: "Latest match result" }),
  ).toHaveLength(1);
  expect(s.latestResult).not.toHaveBeenCalled();
  fireEvent.click(link);
  await screen.findByTestId("personal-result");
  expect(window.location.pathname).toBe("/en/results");
  expect(window.location.search).toBe("");
  expect(s.latestResult).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("grid")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Find a new opponent" }));
  await waitFor(() => expect(window.location.pathname).toBe("/en/queue"));
  s.services.auth.dispose();
});
test("absent and error have separate messages and an explicit retry can show unknown statistics", async () => {
  const s = await setup();
  s.latestResult.mockResolvedValueOnce(null);
  fireEvent.click(
    await screen.findByRole("link", { name: "Latest match result" }),
  );
  await screen.findByText("No recent saved result is available.");
  expect(screen.queryByTestId("personal-result")).toBeNull();
  s.latestResult.mockRejectedValueOnce(Error("offline"));
  fireEvent.click(screen.getByRole("button", { name: "Check stored result" }));
  await screen.findByText("The result could not be checked. Try again.");
  const unknown = {
    ...personalResult(),
    own: null,
    end_elapsed_ms: null,
    result: { reason: "server_failure", outcome: "abort", completed: false },
  } as const;
  s.latestResult.mockResolvedValueOnce(unknown);
  fireEvent.click(screen.getByRole("button", { name: "Check stored result" }));
  await screen.findByTestId("personal-result");
  expect(
    screen.getByText("Statistics for this match are unavailable."),
  ).toBeTruthy();
  s.services.auth.invalidate();
  await waitFor(() =>
    expect(screen.queryByTestId("personal-result")).toBeNull(),
  );
  s.services.auth.dispose();
});
test("a guest result page only requests login and never reads or creates a result account", async () => {
  const s = await setup(true, "/en/results");
  await screen.findByRole("link", { name: "Use an account you already have" });
  expect(s.latestResult).not.toHaveBeenCalled();
  expect(screen.queryByTestId("personal-result")).toBeNull();
  s.services.auth.dispose();
});
