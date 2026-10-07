import { createDailyRecords } from "../src/services/daily-records";
export function dailyTestPorts() {
  return {
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
