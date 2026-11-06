import { dailyTestPorts } from "../test/daily-ports";
import { render, screen, fireEvent, waitFor } from "@testing-library/preact";
import { expect, test } from "vitest";
import { App } from "./App";
import { createI18n } from "./services/i18n";
import { createNavigation } from "./services/navigation";
import { createPreferences } from "./services/preferences";
import type { AppServices } from "./services/ports";
import { createLearning } from "./services/learning";
async function setup(path = "/en/", firstVisit = false) {
  window.history.replaceState(null, "", path);
  const services: AppServices = {
    ...dailyTestPorts(),
    i18n: await createI18n("en"),
    navigation: createNavigation(window),
    preferences: createPreferences(),
    learning: createLearning(),
    trainingCore: async () => {
      throw new Error("unavailable");
    },
    practiceCore: async () => {
      throw new Error("unavailable");
    },
    boardRenderer: async () => {
      throw new Error("unavailable");
    },
  };
  if (!firstVisit) services.learning.mark("skipped");
  render(<App services={services} />);
  return services;
}
test("first visit starts training while skip preserves invitation and replay remains available", async () => {
  const services = await setup("/en/?code=ABCD1234#invitation", true);
  await waitFor(() => expect(window.location.pathname).toBe("/en/tutorial"));
  expect(window.location.search).toContain("code=ABCD1234");
  expect(window.location.hash).toBe("#invitation");
  fireEvent.click(await screen.findByRole("button", { name: "Skip tutorial" }));
  await waitFor(() => expect(window.location.pathname).toBe("/en/"));
  expect(services.learning.read()).toBe("skipped");
  services.navigation.go("/tutorial");
  expect(
    await screen.findByRole("heading", {
      name: "Try an attack and accusation",
    }),
  ).toBeTruthy();
});
test("boots the localized product with a language control and no password collection", async () => {
  await setup();
  expect(screen.getByRole("combobox", { name: "Language" }).isConnected).toBe(
    true,
  );
  expect(screen.getByRole("link", { name: "Liar Sweeper" }).isConnected).toBe(
    true,
  );
  expect(screen.queryByLabelText(/password/i)).toBeNull();
});
test("language changes preserve the current route, invitation query and hash", async () => {
  const services = await setup("/en/settings?code=ABCD1234#preferences");
  fireEvent.change(screen.getAllByRole("combobox", { name: "Language" })[0], {
    target: { value: "ko" },
  });
  await waitFor(() => expect(document.documentElement.lang).toBe("ko"));
  expect(window.location.pathname).toBe("/ko/settings");
  expect(window.location.search).toBe("?code=ABCD1234");
  expect(window.location.hash).toBe("#preferences");
  expect(services.preferences.read().locale).toBe("ko");
});

test("home rule links show the public Rust snapshot and browser back restores the home", async () => {
  await setup();
  fireEvent.click(screen.getAllByRole("link", { name: "Learn the rules" })[0]);
  await waitFor(() =>
    expect(
      screen.getByRole("heading", { level: 1, name: "How to play" })
        .isConnected,
    ).toBe(true),
  );
  expect(screen.getByText(/216 safe squares/).textContent).toContain("4 min");
  window.history.back();
  await waitFor(() =>
    expect(screen.getByRole("heading", { level: 1 }).textContent).toContain(
      "win with logic",
    ),
  );
});
test("settings update accessible controls and unknown routes render a clear 404", async () => {
  const services = await setup("/en/settings");
  fireEvent.change(screen.getByRole("checkbox", { name: "High contrast" }), {
    target: { checked: true },
  });
  fireEvent.change(screen.getByRole("checkbox", { name: "Reduce motion" }), {
    target: { checked: true },
  });
  fireEvent.change(screen.getByRole("combobox", { name: "Board zoom" }), {
    target: { value: "1.5" },
  });
  expect(services.preferences.read()).toEqual({
    contrast: "high",
    motion: "reduce",
    zoom: 1.5,
  });
  await waitFor(() =>
    expect(document.documentElement.dataset.contrast).toBe("high"),
  );
  services.navigation.go("/queue");
  await waitFor(() =>
    expect(
      screen.getByRole("heading", {
        level: 1,
        name: "Online play is unavailable",
      }).isConnected,
    ).toBe(true),
  );
  fireEvent.click(screen.getByRole("link", { name: "Home" }));
  await waitFor(() =>
    expect(screen.getByRole("heading", { level: 1 }).textContent).toContain(
      "win with logic",
    ),
  );
  services.navigation.go("/unknown");
  await waitFor(() =>
    expect(
      screen.getByRole("heading", {
        level: 1,
        name: "Page not found",
      }).isConnected,
    ).toBe(true),
  );
});

test("unknown addresses show a private 404 with fixed locale links and never reflect the input", async () => {
  await setup("/en/missing?email=private-value#secret-fragment");
  expect(
    await screen.findByRole("heading", { name: "Page not found" }),
  ).toBeTruthy();
  expect(screen.getByText("404", { exact: true })).toBeTruthy();
  expect(document.activeElement).toBe(
    screen.getByRole("heading", { name: "Page not found" }),
  );
  expect(document.body.textContent).not.toContain("private-value");
  const home = screen.getByRole("link", { name: "Back to home", exact: true });
  expect(home.getAttribute("href")).toBe("/en/");
  await waitFor(() => expect(document.title).toContain("404"));
  expect(
    document.querySelector('meta[name="robots"]')?.getAttribute("content"),
  ).toContain("noindex");
});
test("the forbidden document explains access without inventing a login or exposing account data", async () => {
  await setup("/en/403?token=private-value");
  expect(
    await screen.findByRole("heading", { name: "Access denied" }),
  ).toBeTruthy();
  expect(
    screen
      .getByRole("link", { name: "View account", exact: true })
      .getAttribute("href"),
  ).toBe("/en/me/account");
  expect(document.body.textContent).not.toContain("private-value");
});

test("first visits keep the error page and head state is restored when returning to a normal document", async () => {
  const services = await setup("/en/not-a-page", true);
  await screen.findByRole("heading", { name: "Page not found" });
  expect(window.location.pathname).toBe("/en/not-a-page");
  await waitFor(() => expect(document.title).toContain("404"));
  services.navigation.go("/rules");
  await waitFor(() =>
    expect(document.title).toBe("How to play | Liar Sweeper"),
  );
  expect(document.querySelector('meta[name="robots"]')).toBeNull();
  services.navigation.go("/me/privacy");
  await waitFor(() =>
    expect(
      document.querySelector('meta[name="robots"]')?.getAttribute("content"),
    ).toContain("noindex"),
  );
  services.navigation.go("/unknown-again");
  await screen.findByRole("heading", { name: "Page not found" });
  expect(
    document.querySelector('meta[name="robots"]')?.getAttribute("content"),
  ).toContain("noindex");
  services.navigation.go("/rules");
  await waitFor(() =>
    expect(document.querySelector('meta[name="robots"]')).toBeNull(),
  );
});
