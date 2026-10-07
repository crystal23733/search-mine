import { createDailyRecords } from "../src/services/daily-records";
import { createActivity } from "../src/services/activity";
import { createOffline } from "../src/services/offline";
export function dailyTestPorts() {
  const activity = createActivity();
  return {
    activity,
    offline: createOffline(activity),
    dailyCore: async () => {
      throw Error("unavailable");
    },
    dailyRecords: createDailyRecords(),
    wallClock: () => Date.parse("2026-10-07T23:59:59Z"),
    share: {
      copy: async () => {},
      nativeShare: async () => {},
      download: async () => {},
    },
  };
}
