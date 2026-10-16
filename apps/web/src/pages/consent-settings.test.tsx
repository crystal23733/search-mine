import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/preact";
import { afterEach, expect, test, vi } from "vitest";
import { Ui } from "../ui/context";
import { SettingsPage } from "./SettingsPage";
import { dailyTestPorts } from "../../test/daily-ports";
import {
  createConsent,
  type CmpPort,
  type ConsentPort,
} from "../services/consent";
import { createI18n } from "../services/i18n";
import { createNavigation } from "../services/navigation";
import { createPreferences } from "../services/preferences";
import { createLearning } from "../services/learning";
import type { AppServices } from "../services/ports";
import { LOCALES, type Locale } from "../services/locale";
const owned: ConsentPort[] = [];
afterEach(() => {
  for (const consent of owned.splice(0)) consent.dispose();
});
async function setup(cmp?: CmpPort, locale: Locale = "en") {
  const consent = createConsent(cmp);
  owned.push(consent);
  const services: AppServices = {
    ...dailyTestPorts(),
    consent,
    i18n: await createI18n(locale),
    navigation: createNavigation(window),
    preferences: createPreferences(),
    learning: createLearning(),
    practiceCore: async () => {
      throw Error("unused");
    },
    trainingCore: async () => {
      throw Error("unused");
    },
    boardRenderer: async () => {
      throw Error("unused");
    },
  };
  const rendered = render(
    <Ui.Provider
      value={{
        locale,
        services,
        preferences: services.preferences.read(),
        selectLocale: () => {},
        t: (key, data) => services.i18n.t(locale, key, data),
      }}
    >
      <SettingsPage />
    </Ui.Provider>,
  );
  return { services, ...rendered };
}

test("settings reopens a CMP explicitly, blocks purposes while pending and keeps revocation after completion", async () => {
  let choice: unknown = {
    status: "valid",
    analytics: true,
    advertising: false,
  };
  let changed = () => {};
  let finish!: () => void;
  const cmp: CmpPort = {
    read: () => choice,
    subscribe: (listener) => {
      changed = listener;
      return () => {};
    },
    reopen: vi.fn(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    ),
  };
  const s = await setup(cmp);
  const button = screen.getByRole("button", { name: "Reopen consent choices" });
  const initial = screen.getByRole("region", { name: "Consent settings" });
  expect(initial.querySelectorAll("dd")[0].textContent).toBe("Allowed");
  expect(initial.querySelectorAll("dd")[1].textContent).toBe("Blocked");
  expect(cmp.reopen).not.toHaveBeenCalled();
  fireEvent.click(button);
  await waitFor(() => expect(cmp.reopen).toHaveBeenCalledOnce());
  const region = screen.getByRole("region", { name: "Consent settings" });
  expect(region.querySelectorAll("dd")[0].textContent).toBe("Blocked");
  expect(region.querySelectorAll("dd")[1].textContent).toBe("Blocked");
  expect(s.services.activity.read()).toEqual({ busy: false, locked: false });
  choice = { status: "denied" };
  changed();
  finish();
  await waitFor(() =>
    expect(
      within(region)
        .getByRole("button", { name: "Reopen consent choices" })
        .hasAttribute("disabled"),
    ).toBe(false),
  );
  expect(region.querySelectorAll("dd")[0].textContent).toBe("Blocked");
  expect(region.querySelectorAll("dd")[1].textContent).toBe("Blocked");
  s.unmount();
});

test.each(LOCALES)(
  "%s unconfigured choices are translated, disabled and keep both purposes blocked",
  async (locale) => {
    const s = await setup(undefined, locale);
    const t = (key: Parameters<typeof s.services.i18n.t>[1]) =>
      s.services.i18n.t(locale, key);
    const region = screen.getByRole("region", { name: t("consent") });
    expect(within(region).getByRole("status").textContent).toBe(
      t("consent.state.unavailable"),
    );
    expect(within(region).getByText(t("consent.analytics"))).toBeTruthy();
    expect(within(region).getByText(t("consent.advertising"))).toBeTruthy();
    expect(
      [...region.querySelectorAll("dd")].map((el) => el.textContent),
    ).toEqual([t("consent.blocked"), t("consent.blocked")]);
    expect(
      within(region)
        .getByRole("button", { name: t("consent.reopen") })
        .hasAttribute("disabled"),
    ).toBe(true);
    expect(s.services.activity.read()).toEqual({ busy: false, locked: false });
  },
);

test("failed reopening shows a safe error and blocks old permissions without taking the game lock", async () => {
  const s = await setup({
    read: () => ({ status: "valid", analytics: true, advertising: true }),
    subscribe: () => () => {},
    reopen: async () => {
      throw Error("private vendor payload");
    },
  });
  fireEvent.click(
    screen.getByRole("button", { name: "Reopen consent choices" }),
  );
  expect((await screen.findByRole("alert")).textContent).toBe(
    s.services.i18n.t("en", "consent.reopenFailed"),
  );
  const region = screen.getByRole("region", { name: "Consent settings" });
  expect(
    [...region.querySelectorAll("dd")].map((el) => el.textContent),
  ).toEqual(["Blocked", "Blocked"]);
  expect(region.textContent).not.toContain("private vendor payload");
  expect(s.services.activity.read()).toEqual({ busy: false, locked: false });
});

test("cancel and unmount abort an ignored CMP request; late completion cannot update the removed screen", async () => {
  const requests: Array<{ signal: AbortSignal; finish: () => void }> = [];
  const s = await setup({
    read: () => ({ status: "unknown" }),
    subscribe: () => () => {},
    reopen: (signal) =>
      new Promise<void>((finish) => requests.push({ signal, finish })),
  });
  fireEvent.click(
    screen.getByRole("button", { name: "Reopen consent choices" }),
  );
  await waitFor(() => expect(requests).toHaveLength(1));
  fireEvent.click(screen.getByRole("button", { name: "Cancel opening" }));
  expect(requests[0].signal.aborted).toBe(true);
  await waitFor(() =>
    expect(
      screen
        .getByRole("button", { name: "Reopen consent choices" })
        .hasAttribute("disabled"),
    ).toBe(false),
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Reopen consent choices" }),
  );
  await waitFor(() => expect(requests).toHaveLength(2));
  s.unmount();
  expect(requests[1].signal.aborted).toBe(true);
  requests[0].finish();
  requests[1].finish();
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  expect(s.services.consent.read().working).toBe(false);
  expect(screen.queryByRole("alert")).toBeNull();
  expect(s.services.activity.read()).toEqual({ busy: false, locked: false });
});
