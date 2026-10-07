import {
  WorkerPracticeCore,
  WorkerTrainingCore,
  type PracticeCore,
  type TrainingCore,
} from "@liar/core-bridge";
function worker(): Worker {
  return new Worker(
    new URL("../../../../packages/core-bridge/src/worker.ts", import.meta.url),
    { type: "module" },
  );
}
export function createPracticeCore(): PracticeCore {
  return new WorkerPracticeCore(worker());
}
export function createTrainingCore(): TrainingCore {
  return new WorkerTrainingCore(worker());
}
