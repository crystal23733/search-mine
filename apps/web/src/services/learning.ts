export type LearningStatus = "new" | "skipped" | "complete";
export interface LearningPort {
  read(): LearningStatus;
  mark(status: "skipped" | "complete"): void;
  persistent(): boolean;
}
export const LEARNING_KEY = "liar.tutorial.v1";
export function createLearning(
  storage?: Pick<Storage, "getItem" | "setItem">,
): LearningPort {
  let status: LearningStatus = "new";
  let persistent = Boolean(storage);
  let raw: string | null | undefined;
  try {
    raw = storage?.getItem(LEARNING_KEY);
  } catch {
    persistent = false;
  }
  if (raw && raw.length <= 2048) {
    try {
      const value: unknown = JSON.parse(raw);
      if (value === "complete" || value === "skipped") status = value;
    } catch {
      /* Invalid data leaves the first-visit state. */
    }
  }
  return {
    read: () => status,
    persistent: () => persistent,
    mark: (next) => {
      status = next;
      try {
        storage?.setItem(LEARNING_KEY, JSON.stringify(status));
        persistent = Boolean(storage);
      } catch {
        persistent = false;
      }
    },
  };
}
