import { useState } from "preact/hooks";
import type { LobbyDifficulty } from "@liar/protocol";
import type { OnlinePanelProps } from "./OnlineEntry";
import { useUi } from "../ui/context";
import { Button } from "../ui/atoms/Button";
import { Card } from "../ui/atoms/Card";
export function QueuePanel({ controller, state, locked }: OnlinePanelProps) {
  const { t, locale } = useUi();
  const [difficulty, setDifficulty] = useState<LobbyDifficulty>("normal");
  const selectedDifficulty =
    state.status === "ready" ? difficulty : (state.difficulty ?? difficulty);
  return (
    <div class="queue-panel">
      <Card>
        {state.lobby?.state.type === "queued" ? (
          <>
            <div
              class="queue-clock"
              style={{
                "--queue-progress": `${Math.min(100, state.waitMs / 100)}%`,
              }}
            >
              <div>
                <output aria-live="off" aria-label={t("queue.searching")}>
                  {new Intl.NumberFormat(locale, {
                    minimumIntegerDigits: 2,
                  }).format(Math.ceil(state.waitMs / 1000))}
                </output>
                <span>
                  {t("queue.botIn", {
                    seconds: Math.ceil(state.waitMs / 1000),
                  })}
                </span>
              </div>
            </div>
            <h2>{t("queue.searching")}</h2>
          </>
        ) : (
          <output class="status-message">
            {t(
              state.status === "connecting"
                ? "queue.connecting"
                : state.status === "waiting"
                  ? "queue.preparing"
                  : state.status === "loading"
                    ? "match.loading"
                    : "queue.hint",
            )}
          </output>
        )}
        <fieldset
          class="queue-difficulties"
          disabled={state.status !== "ready" || state.working || locked}
        >
          <legend>{t("match.difficulty")}</legend>
          {(["easy", "normal", "hard"] as const).map((value) => (
            <Button
              key={value}
              aria-pressed={selectedDifficulty === value}
              onClick={() => setDifficulty(value)}
            >
              {t(`match.${value}`)}
            </Button>
          ))}
        </fieldset>
        {state.status === "ready" && (
          <Button
            variant="primary"
            disabled={state.working || locked}
            onClick={() => void controller.join(difficulty)}
          >
            {t("queue.find")}
          </Button>
        )}
        {state.status === "waiting" && (
          <>
            <p>{t("queue.hint")}</p>
            <Button
              disabled={state.working}
              onClick={() => void controller.cancel()}
            >
              {t("queue.cancel")}
            </Button>
          </>
        )}
      </Card>
    </div>
  );
}
