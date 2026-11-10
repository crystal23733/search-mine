import { createInstall } from "../src/services/install";
import { createDailyRecords } from "../src/services/daily-records";
import { createActivity } from "../src/services/activity";
import { createOffline } from "../src/services/offline";
import { createConsent } from "../src/services/consent";
import { guestAuthPorts } from "./guest-auth";
import { createPendingSubmissions } from "../src/services/pending-submissions";
export function dailyTestPorts() {
  const activity = createActivity();
  const auth = guestAuthPorts();
  const records = createDailyRecords();
  return {
    install: createInstall(),
    consent: createConsent(),
    consentControl: {
      read: async (owner: { accountId: string }) => ({
        v: 1,
        account_id: owner.accountId,
        revision: null,
        observed_at: 1500,
        consent: {
          cmp_available: false,
          status: "unknown" as const,
          analytics: "unknown" as const,
          advertising: "unknown" as const,
          checked_at: null,
          expires_at: null,
        },
      }),
      refresh: async () => {
        throw Error("unavailable");
      },
      withdraw: async () => {
        throw Error("unavailable");
      },
    },
    me: {
      overview: async () => {
        throw Error("unavailable");
      },
      history: async () => {
        throw Error("unavailable");
      },
    },
    dailyChallenge: {
      start: async () => {
        throw Error("unavailable");
      },
      heartbeat: async () => {
        throw Error("unavailable");
      },
      definition: async () => {
        throw Error("unavailable");
      },
      own: async () => {
        throw Error("unavailable");
      },
      submitTimed: async () => {
        throw Error("unavailable");
      },
    },
    dailyRankings: async () => {
      throw Error("unavailable");
    },
    online: {
      latestResult: async () => {
        throw Error("unavailable");
      },
      result: async () => {
        throw Error("unavailable");
      },
      lobby: async () => {
        throw Error("unavailable");
      },
      connect: async () => {
        throw Error("unavailable");
      },
    },
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
    },
  };
}
