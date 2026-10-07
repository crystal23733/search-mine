import type { DailyCore } from "@liar/core-bridge";
import {
  DAILY_SEED_VERSION,
  type DailyReplay,
  type DailyView,
} from "@liar/protocol";
import { LocalController, browserClock } from "./local-session";
export function utcDailyDate(time: number): string {
  try {
    const date = new Date(time).toISOString();
    return /^\d{4}-/.test(date) ? date.slice(0, 10) : "";
  } catch {
    return "";
  }
}
export function createDailyRun(
  date: string,
  factory: () => Promise<DailyCore>,
) {
  let core: DailyCore | undefined;
  let replay: Promise<DailyReplay> | undefined;
  let attemptId = "";
  const controller = new LocalController<DailyView>(
    async () => {
      attemptId = crypto.randomUUID();
      core = await factory();
      const session = core;
      return {
        init: () => session.init(date, DAILY_SEED_VERSION),
        advance: (time) => session.advance(time),
        step: (input, time) => session.step(input, time),
        dispose: () => {
          if (replay)
            void replay.catch(() => {}).finally(() => session.dispose());
          else session.dispose();
        },
      };
    },
    browserClock,
    (view) => ["finished", "aborted", "cancelled"].includes(view.phase),
  );
  return {
    controller,
    attemptId: () => attemptId,
    replay: () =>
      (replay ??= core ? core.replay() : Promise.reject(Error("unavailable"))),
  };
}
