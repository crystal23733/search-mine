import { act, fireEvent, render, screen } from "@testing-library/preact";
import { expect, test, vi } from "vitest";
import { InstallCard } from "../ui/organisms/InstallCard";
import { Ui } from "../ui/context";
import { createInstall } from "../services/install";
import { createActivity } from "../services/activity";
import { createI18n } from "../services/i18n";
import { createPreferences } from "../services/preferences";
import { LOCALES, type Locale } from "../services/locale";
import type { AppServices } from "../services/ports";
async function setup(locale: Locale = "en") {
  const i18n = await createI18n(locale),
    install = createInstall(window),
    activity = createActivity();
  const t = (key: Parameters<typeof i18n.t>[1]) => i18n.t(locale, key);
  const mounted = render(
    <Ui.Provider
      value={{
        locale,
        t,
        selectLocale: () => {},
        preferences: createPreferences().read(),
        services: { install, activity } as AppServices,
      }}
    >
      <InstallCard />
    </Ui.Provider>,
  );
  return {
    ...mounted,
    t,
    activity,
    install,
    dispose: () => {
      mounted.unmount();
      install.dispose();
    },
  };
}
for (const locale of LOCALES)
  test(`installation stays optional and dismissal leaves usable in-page guidance in ${locale}`, async () => {
    const s = await setup(locale);
    expect(
      screen.getByRole("heading", { name: s.t("install.title") }),
    ).toBeTruthy();
    expect(screen.getByText(s.t("install.guide"))).toBeTruthy();
    expect(screen.queryByRole("button")).toBeNull();
    let resolve!: (value: { outcome: string }) => void;
    const userChoice = new Promise<{ outcome: string }>((done) => {
      resolve = done;
    });
    const prompt = vi.fn(async () => {});
    act(() => {
      window.dispatchEvent(
        Object.assign(new Event("beforeinstallprompt", { cancelable: true }), {
          userChoice,
          prompt,
        }),
      );
    });
    const button = screen.getByRole("button", { name: s.t("install.action") });
    const release = s.activity.hold();
    await act(async () => {});
    expect((button as HTMLButtonElement).disabled).toBe(true);
    act(release);
    fireEvent.click(button);
    expect(prompt).toHaveBeenCalledTimes(1);
    await act(async () => {
      resolve({ outcome: "dismissed" });
    });
    expect(await screen.findByText(s.t("install.dismissed"))).toBeTruthy();
    expect(screen.getByText(s.t("install.guide"))).toBeTruthy();
    expect(screen.getByText(s.t("install.network"))).toBeTruthy();
    s.dispose();
  });
test("a browser installation notification changes the card and removes the prompt action", async () => {
  const s = await setup();
  act(() => {
    window.dispatchEvent(new Event("appinstalled"));
  });
  expect(screen.getByText(s.t("install.installed"))).toBeTruthy();
  expect(screen.queryByRole("button")).toBeNull();
  s.dispose();
});
