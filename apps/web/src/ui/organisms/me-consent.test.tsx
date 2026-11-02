import { act, render, screen, within } from "@testing-library/preact";
import { expect, test, vi } from "vitest";
import { Ui } from "../context";
import { MeConsent } from "./MeConsent";
import { overview, ME_NOW } from "../../../test/me-data";
import { dailyTestPorts } from "../../../test/daily-ports";
import { createI18n } from "../../services/i18n";
import { createNavigation } from "../../services/navigation";
import { createPreferences } from "../../services/preferences";
import { createLearning } from "../../services/learning";
import type { AppServices } from "../../services/ports";
test("consent display expires using monotonic request age and never preserves an old grant", async () => {
  const services: AppServices = {
    ...dailyTestPorts(),
    i18n: await createI18n("en"),
    navigation: createNavigation(window),
    preferences: createPreferences(),
    learning: createLearning(),
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
  const value = overview();
  value.consent = {
    cmp_available: false,
    status: "current",
    analytics: "granted",
    advertising: "denied",
    checked_at: ME_NOW - 1,
    expires_at: ME_NOW + 1,
  };
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
  try {
    render(
      <Ui.Provider
        value={{
          locale: "en",
          services,
          preferences: services.preferences.read(),
          selectLocale: () => {},
          t: (key, data) => services.i18n.t("en", key, data),
        }}
      >
        <MeConsent value={value} startedAt={performance.now() - 500} />
      </Ui.Provider>,
    );
    expect(screen.getByText("Current confirmation")).toBeTruthy();
    expect(screen.getByText("Allowed", { exact: true })).toBeTruthy();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(499);
    });
    expect(screen.getByText("Allowed", { exact: true })).toBeTruthy();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
    expect(screen.getByText("Confirmation expired")).toBeTruthy();
    expect(screen.queryByText("Allowed", { exact: true })).toBeNull();
    expect(
      within(screen.getByTestId("me-server-consent")).getAllByText(
        "Not confirmed",
      ),
    ).toHaveLength(2);
  } finally {
    vi.useRealTimers();
  }
});
