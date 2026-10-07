import { render, screen, fireEvent, waitFor } from "@testing-library/preact";
import { expect, test } from "vitest";
import { App } from "./App";
import { createI18n } from "./services/i18n";
import { createNavigation } from "./services/navigation";
import { createPreferences } from "./services/preferences";
import type { AppServices } from "./services/ports";
async function setup(path = "/en/") {
  window.history.replaceState(null, "", path);
  const services: AppServices = {
    i18n: await createI18n("en"),
    navigation: createNavigation(window),
    preferences: createPreferences(),
    practiceCore: async () => {
      throw new Error("unavailable");
    },
  };
  render(<App services={services} />);
  return services;
}
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
test("settings update accessible controls while online and unknown routes stay unavailable", async () => {
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
        name: "This page is unavailable",
      }).isConnected,
    ).toBe(true),
  );
});
