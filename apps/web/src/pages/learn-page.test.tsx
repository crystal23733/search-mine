import { render, screen, fireEvent, waitFor } from "@testing-library/preact";
import { expect, test, vi } from "vitest";
import { App } from "../App";
import { dailyTestPorts } from "../../test/daily-ports";
import { createI18n } from "../services/i18n";
import { createNavigation } from "../services/navigation";
import { createPreferences } from "../services/preferences";
import { createLearning, type LearningStatus } from "../services/learning";
import type { AppServices } from "../services/ports";
async function setup(state: LearningStatus = "new", denied = false) {
  window.history.replaceState(
    null,
    "",
    "/en/learn?token=private-value#private-fragment",
  );
  const learning = denied
    ? createLearning({
        getItem: () => {
          throw new DOMException("Denied");
        },
        setItem: () => {
          throw new DOMException("Denied");
        },
      })
    : createLearning();
  if (state !== "new") learning.mark(state);
  const mark = vi.spyOn(learning, "mark");
  const unavailable = vi.fn(async () => {
    throw Error("A guide must not initialize a game");
  });
  const services: AppServices = {
    ...dailyTestPorts(),
    i18n: await createI18n("en"),
    navigation: createNavigation(window),
    preferences: createPreferences(),
    learning,
    publicSiteOrigin: "https://sweeper.example.test",
    practiceCore: unavailable,
    trainingCore: unavailable,
    dailyCore: unavailable,
    boardRenderer: unavailable,
  };
  render(<App services={services} />);
  return { services, mark, unavailable };
}
for (const state of ["new", "skipped", "complete"] as const)
  test(`guide reading and locale changes preserve ${state} without game initialization`, async () => {
    const s = await setup(state);
    await screen.findByRole("heading", { name: "Tutorial guide", level: 1 });
    expect(window.location.pathname).toBe("/en/learn");
    expect(screen.queryByRole("grid")).toBeNull();
    const start = screen.getByRole("link", {
      name: "Start the tutorial",
      exact: true,
    });
    expect(start.getAttribute("href")).toBe("/en/tutorial");
    expect(
      screen
        .getByRole("link", { name: "How to play", exact: true })
        .getAttribute("href"),
    ).toBe("/en/rules");
    expect(
      document.querySelector('meta[name="robots"]')?.getAttribute("content"),
    ).toContain("noindex");
    expect(
      document.head.innerHTML + document.querySelector("main")?.innerHTML,
    ).not.toContain("private-value");
    fireEvent.change(
      screen.getByRole("combobox", { name: "Language", exact: true }),
      { target: { value: "ko" } },
    );
    await screen.findByRole("heading", { name: "튜토리얼 안내", level: 1 });
    expect(
      screen
        .getByRole("link", { name: "튜토리얼 시작", exact: true })
        .getAttribute("href"),
    ).toBe("/ko/tutorial");
    await waitFor(() => expect(document.documentElement.lang).toBe("ko"));
    expect(s.services.learning.read()).toBe(state);
    expect(s.mark).not.toHaveBeenCalled();
    expect(s.unavailable).not.toHaveBeenCalled();
  });
test("storage denial keeps the guide readable and does not mark learning", async () => {
  const s = await setup("new", true);
  await screen.findByRole("heading", { name: "Tutorial guide", level: 1 });
  expect(s.services.learning.persistent()).toBe(false);
  expect(s.services.learning.read()).toBe("new");
  expect(s.mark).not.toHaveBeenCalled();
  expect(s.unavailable).not.toHaveBeenCalled();
});
