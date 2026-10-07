import { fireEvent, render, screen, waitFor } from "@testing-library/preact";
import { expect, test, vi } from "vitest";
import {
  DEFAULT_RULES,
  type GameView,
  type LocalInput,
  type LocalStep,
  type TutorialView,
} from "@liar/protocol";
import { App } from "../App";
import { createI18n } from "../services/i18n";
import { createNavigation } from "../services/navigation";
import { createPreferences } from "../services/preferences";
import { createLearning } from "../services/learning";
import type { AppServices } from "../services/ports";
function fixture(): GameView {
  return {
    v: 1,
    rules: DEFAULT_RULES,
    revision: 0,
    phase: "playing",
    countdown_ms: 0,
    remaining_ms: 240000,
    own: {
      cells: Array.from({ length: 256 }, (_, cell) => ({
        cell,
        state: cell === 0 ? "safe" : "closed",
        number: cell === 0 ? 0 : null,
        flagged: false,
      })),
      gauge: 20,
      stun_ms: 0,
      history: [],
      stats: { mistakes: 0, accusation_attempts: 0, correct_accusations: 0 },
    },
    opponent: { opened_safe: 1, stun_ms: 0 },
    result: null,
  };
}
async function services(path: string): Promise<AppServices> {
  window.history.replaceState(null, "", path);
  const learning = createLearning();
  learning.mark("skipped");
  return {
    i18n: await createI18n("en"),
    navigation: createNavigation(window),
    preferences: createPreferences(),
    learning,
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
}
test("local practice displays authoritative rejection and result, replaces difficulty sessions and frees the worker", async () => {
  const ports = await services(
    "/en/practice?difficulty=easy&code=ABCD1234#invite",
  );
  const cores: Array<{
    init: ReturnType<typeof vi.fn>;
    dispose: ReturnType<typeof vi.fn>;
  }> = [];
  const actions: LocalInput[] = [];
  ports.practiceCore = async () => {
    let view = fixture();
    const init = vi.fn(async () => view),
      dispose = vi.fn();
    cores.push({ init, dispose });
    return {
      init,
      snapshot: async () => view,
      advance: async () => view,
      dispose,
      step: async (input) => {
        actions.push(input);
        if (input.action.type === "attack")
          view = {
            ...view,
            phase: "finished",
            result: { outcome: "draw", reason: "timeout", completed: true },
          };
        const result: LocalStep = {
          view,
          ack: {
            command_id: input.command_id,
            revision: 1,
            duplicate: false,
            status: input.action.type === "attack" ? "applied" : "rejected",
            error: input.action.type === "attack" ? null : "already_open",
          },
        };
        return result;
      },
    };
  };
  const mounted = render(<App services={ports} />);
  await screen.findByRole("grid");
  expect(cores[0].init.mock.calls[0][1]).toBe("easy");
  fireEvent.click(screen.getByRole("gridcell", { name: "B1, closed" }));
  expect(
    await screen.findByText(
      "That action could not be applied. Try another square.",
    ),
  ).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Attack", exact: true }));
  expect(await screen.findByRole("heading", { name: "Draw" })).toBeTruthy();
  expect(actions.map((input) => input.command_id)).toEqual([1, 2]);
  expect(
    screen
      .getByRole("button", { name: "Attack", exact: true })
      .hasAttribute("disabled"),
  ).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "New match" }));
  await waitFor(() => expect(cores).toHaveLength(2));
  expect(cores[0].dispose).toHaveBeenCalledTimes(1);
  fireEvent.change(screen.getByLabelText("Bot difficulty"), {
    target: { value: "hard" },
  });
  await waitFor(() => expect(cores).toHaveLength(3));
  expect(cores[2].init.mock.calls[0][1]).toBe("hard");
  expect(window.location.search).toContain("code=ABCD1234");
  expect(window.location.hash).toBe("#invite");
  mounted.unmount();
  await waitFor(() => expect(cores[2].dispose).toHaveBeenCalledTimes(1));
});
test("training follows the public expected action, marks completion and replays without creating an account", async () => {
  const ports = await services(
    "/en/tutorial?return=%2Fpractice&difficulty=hard&code=ABCD1234#invite",
  );
  const allDispose: ReturnType<typeof vi.fn>[] = [];
  ports.trainingCore = async () => {
    let view: TutorialView = {
      game: fixture(),
      stage: "open",
      expected: { type: "open", cell: 8 },
    };
    const dispose = vi.fn();
    allDispose.push(dispose);
    return {
      init: async () => view,
      snapshot: async () => view,
      advance: async () => view,
      dispose,
      step: async (input) => {
        view = { ...view, stage: "complete", expected: null };
        return {
          view,
          ack: {
            command_id: input.command_id,
            revision: 1,
            status: "applied",
            error: null,
            duplicate: false,
          },
        };
      },
    };
  };
  const mounted = render(<App services={ports} />);
  fireEvent.click(await screen.findByRole("button", { name: "Open I1" }));
  expect(
    await screen.findByRole("heading", { name: "You reflected the attack!" }),
  ).toBeTruthy();
  await waitFor(() => expect(ports.learning.read()).toBe("complete"));
  fireEvent.click(screen.getByRole("button", { name: "Replay tutorial" }));
  await screen.findByRole("button", { name: "Open I1" });
  expect(allDispose[0]).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole("button", { name: "Skip tutorial" }));
  await waitFor(() => expect(window.location.pathname).toBe("/en/practice"));
  expect(ports.learning.read()).toBe("complete");
  expect(window.location.search).not.toContain("return=");
  expect(window.location.search).toContain("difficulty=hard");
  mounted.unmount();
});
test("failed local initialization stays recoverable by new match", async () => {
  const ports = await services("/en/practice?difficulty=invalid");
  const factory = vi.fn(async () => {
    throw Error("private failure");
  });
  ports.practiceCore = factory;
  const mounted = render(<App services={ports} />);
  expect(await screen.findByRole("alert")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "New match" }));
  await waitFor(() => expect(factory).toHaveBeenCalledTimes(2));
  expect(document.body.textContent).not.toContain("private failure");
  mounted.unmount();
});
