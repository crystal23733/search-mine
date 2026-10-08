import { createDailyRecords } from "../src/services/daily-records";
import { createActivity } from "../src/services/activity";
import { createOffline } from "../src/services/offline";
import { authTestPorts } from "./auth-ports";
import { createPendingSubmissions } from "../src/services/pending-submissions";
export function dailyTestPorts() {
  const activity = createActivity();
  const auth = authTestPorts();
  const records = createDailyRecords();
  return {
    ...auth,
    pendingSubmissions: createPendingSubmissions(records, auth.auth, {
      submit: async () => {
        throw Error("official_submission_unavailable");
      },
    }),
    activity,
    offline: createOffline(activity),
    dailyCore: async () => {
      throw Error("unavailable");
    },
    dailyRecords: records,
    wallClock: () => Date.parse("2026-10-07T23:59:59Z"),
    share: {
      copy: async () => {},
      nativeShare: async () => {},
      download: async () => {},
    },
  };
}
