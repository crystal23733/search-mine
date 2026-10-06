import { WorkerPracticeCore, type PracticeCore } from "@liar/core-bridge";
export function createPracticeCore(): PracticeCore {
  return new WorkerPracticeCore(
    new Worker(
      new URL(
        "../../../../packages/core-bridge/src/worker.ts",
        import.meta.url,
      ),
      { type: "module" },
    ),
  );
}
