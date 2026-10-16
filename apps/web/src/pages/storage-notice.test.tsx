import { fireEvent, render, screen, within } from "@testing-library/preact";
import { expect, test, vi } from "vitest";
import { App } from "../App";
import { dailyTestPorts } from "../../test/daily-ports";
import { createI18n } from "../services/i18n";
import { createNavigation } from "../services/navigation";
import { createPreferences } from "../services/preferences";
import { createLearning } from "../services/learning";
import type { AppServices } from "../services/ports";
import { LOCALES, type Locale } from "../services/locale";

async function setup(locale: Locale = "en") {
  window.history.replaceState(null, "", `/${locale}/`);
  const learning = createLearning();
  learning.mark("skipped");
  const services: AppServices = {
    ...dailyTestPorts(),
    i18n: await createI18n(locale),
    navigation: createNavigation(window),
    preferences: createPreferences(),
    learning,
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
  return services;
}

test("storage notice is reachable and reading or closing it never submits or deletes data", async () => {
  const services = await setup();
  const operations = [
    vi.spyOn(services.dailyRecords, "clear"),
    vi.spyOn(services.pendingSubmissions, "flush"),
    vi.spyOn(services.offline, "clear"),
    vi.spyOn(services.auth, "start"),
    vi.spyOn(services.auth, "logout"),
    vi.spyOn(services.auth, "erase"),
  ];
  render(<App services={services} />);
  fireEvent.click(screen.getByRole("link", { name: "Storage and cookies" }));
  await screen.findByRole("heading", { name: "Storage and cookies", level: 1 });
  const main = screen.getByRole("main");
  expect(within(main).getByText("Security cookies")).toBeTruthy();
  expect(main.textContent).toContain("5 minutes");
  expect(main.textContent).toContain("30 days");
  expect(main.textContent).toContain("30 daily records");
  expect(main.textContent).toContain("not submitted automatically");
  expect(main.textContent).toContain(
    "Official daily play automatically submits after you confirm its start.",
  );
  fireEvent.click(within(main).getByRole("link", { name: "Settings" }));
  const opener = await screen.findByRole("button", {
    name: "Essential storage",
  });
  fireEvent.click(opener);
  const dialog = await screen.findByRole("dialog", {
    name: "Essential storage",
  });
  expect(within(dialog).getByText("Security cookies")).toBeTruthy();
  expect(dialog.textContent).toContain("not submitted automatically");
  fireEvent.click(within(dialog).getByRole("button", { name: "Close" }));
  for (const operation of operations) expect(operation).not.toHaveBeenCalled();
});

test.each(LOCALES)(
  "%s renders each purpose and clearing boundary in its own locale",
  async (locale) => {
    const services = await setup(locale);
    const t = (key: Parameters<typeof services.i18n.t>[1]) =>
      services.i18n.t(locale, key);
    render(<App services={services} />);
    fireEvent.click(screen.getByRole("link", { name: t("storage.pageTitle") }));
    await screen.findByRole("heading", {
      name: t("storage.pageTitle"),
      level: 1,
    });
    const main = screen.getByRole("main");
    for (const purpose of [
      "preferences",
      "records",
      "cache",
      "cookies",
      "optional",
    ] as const) {
      expect(
        within(main).getByText(t(`storage.${purpose}.title`)),
      ).toBeTruthy();
      expect(
        within(main).getByText(t(`storage.${purpose}.description`)),
      ).toBeTruthy();
    }
    expect(main.textContent).toContain(t("storage.manage"));
    expect(main.textContent).toContain(t("storage.siteData"));
    expect(main.textContent).toContain(t("storage.fallback"));
    if (locale !== "en")
      expect(main.textContent).not.toContain("Security cookies");
    expect(main.querySelectorAll("button,input,select")).toHaveLength(0);
  },
);
