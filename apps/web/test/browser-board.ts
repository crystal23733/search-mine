import { dailyTestPorts } from "./daily-ports";
import { createLearning } from "../src/services/learning";
import { h, render } from "preact";
import type { GameView, PublicAction } from "@liar/protocol";
import { Ui, type UiContext } from "../src/ui/context";
import { Board } from "../src/ui/organisms/Board";
import { createI18n } from "../src/services/i18n";
import { createPreferences } from "../src/services/preferences";
import { createNavigation } from "../src/services/navigation";
import { createPixiBoard } from "../src/board/pixi";
export async function mountBoard(initial: GameView) {
  const i18n = await createI18n("en");
  const preferences = createPreferences();
  const ui: UiContext = {
    locale: "en",
    t: (key, data) => i18n.t("en", key, data),
    preferences: preferences.read(),
    selectLocale() {},
    services: {
      ...dailyTestPorts(),
      i18n,
      preferences,
      navigation: createNavigation(window),
      boardRenderer: createPixiBoard,
      learning: createLearning(),
      trainingCore: async () => {
        throw new Error("unavailable");
      },
      practiceCore: async () => {
        throw new Error("unavailable");
      },
    },
  };
  const root = document.getElementById("app")!;
  render(null, root);
  const harness = {
    actions: [] as PublicAction[],
    view: initial,
    update(view: GameView) {
      harness.view = view;
      render(
        h(
          Ui.Provider,
          { value: ui },
          h(
            "main",
            { style: { padding: "16px", maxWidth: "1100px", margin: "auto" } },
            h(Board, {
              view,
              createRenderer: createPixiBoard,
              onAction: (action) => harness.actions.push(action),
            }),
          ),
        ),
        root,
      );
    },
  };
  harness.update({ ...initial, phase: "playing", countdown_ms: 0 });
  return harness;
}
