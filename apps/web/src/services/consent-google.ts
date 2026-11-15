import {
  GOOGLE_AD_PURPOSES,
  GOOGLE_ANALYTICS_PURPOSES,
  GOOGLE_CMP_ID,
  GOOGLE_VENDOR_ID,
  MAX_TCF_CHARS,
  TCF_POLICY_VERSIONS,
  type ConsentEvidence,
} from "@liar/protocol";
import type { ConsentAdapter } from "./consent-browser";
import type { ConsentContextPort, ConsentReceipt } from "./consent-guard";
import type { GooglePublisherClient } from "./ads/google";
type Value = Record<string, unknown>;
type Tcf = (
  command: string,
  version: number,
  callback: (data: unknown, success: boolean) => void,
  parameter?: unknown,
) => void;
type GoogleWindow = Window & {
  __tcfapi?: Tcf;
  googlefc?: {
    callbackQueue: Array<Record<string, () => void>>;
    showRevocationMessage?: () => void;
  };
};
const object = (value: unknown): Value =>
  value && typeof value === "object" && !Array.isArray(value)
    ? (value as Value)
    : {};
const consented = (value: unknown, purposes: readonly number[]) =>
  purposes.every((id) => object(value)[id] === true);
export interface GoogleConsentAdapter extends ConsentAdapter {
  evidence(): ConsentEvidence | undefined;
  subscribeBroadcast(listener: () => void): () => void;
}
export function createGoogleConsentAdapter(
  browser: Window,
  context: ConsentContextPort,
  client: Pick<GooglePublisherClient, "state" | "subscribe">,
  publicDocument: () => boolean,
  navigateToChoices: () => void,
): GoogleConsentAdapter {
  const google = browser as GoogleWindow;
  let disposed = false,
    ready = false,
    queued = false,
    generation = 0,
    revision = 0;
  let listenerId: number | undefined;
  let receipt: ConsentReceipt | undefined,
    evidence: ConsentEvidence | undefined;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const listeners = new Set<() => void>(),
    broadcasts = new Set<() => void>();
  const emit = () => {
    if (!disposed) for (const fn of listeners) fn();
  };
  const clear = () => {
    receipt = undefined;
    evidence = undefined;
    clearTimeout(timer);
  };
  function detach() {
    generation++;
    if (listenerId !== undefined) {
      try {
        google.__tcfapi?.("removeEventListener", 2, () => {}, listenerId);
      } catch {
        /* A retired callback cannot grant permission. */
      }
    }
    listenerId = undefined;
  }
  function attach() {
    detach();
    clear();
    const owner = context.capture(),
      current = generation;
    if (disposed || !ready || !owner || !publicDocument() || !google.__tcfapi) {
      emit();
      return;
    }
    let initial = true;
    try {
      google.__tcfapi("addEventListener", 2, (raw, success) => {
        if (disposed || current !== generation || context.capture() !== owner)
          return;
        const first = initial;
        initial = false;
        const data = object(raw);
        if (
          Number.isSafeInteger(data.listenerId) &&
          Number(data.listenerId) >= 0
        )
          listenerId = Number(data.listenerId);
        clear();
        if (
          success &&
          data.cmpStatus === "loaded" &&
          data.cmpId === GOOGLE_CMP_ID &&
          data.gdprApplies === true &&
          data.isServiceSpecific === true &&
          TCF_POLICY_VERSIONS.some(
            (version) => version === data.tcfPolicyVersion,
          ) &&
          (data.eventStatus === "tcloaded" ||
            data.eventStatus === "useractioncomplete") &&
          typeof data.tcString === "string" &&
          data.tcString.length > 0 &&
          data.tcString.length <= MAX_TCF_CHARS &&
          /^[A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+){0,3}$/.test(data.tcString)
        ) {
          const publisher = object(data.publisher),
            restrictions = object(publisher.restrictions);
          const advertising =
            object(object(data.vendor).consents)[GOOGLE_VENDOR_ID] === true &&
            consented(object(data.purpose).consents, GOOGLE_AD_PURPOSES) &&
            GOOGLE_AD_PURPOSES.every((id) => {
              const restriction = object(restrictions[id])[GOOGLE_VENDOR_ID];
              return restriction === undefined || restriction === 1;
            });
          const now = Date.now();
          receipt = {
            status: "valid",
            context: owner,
            revision: ++revision,
            validFrom: now,
            expiresAt: now + 60000,
            analytics: consented(publisher.consents, GOOGLE_ANALYTICS_PURPOSES),
            advertising,
          };
          evidence = { provider: "google_tcf_v2", tc_string: data.tcString };
          timer = setTimeout(attach, 60000);
        }
        emit();
        if (!first && data.eventStatus === "useractioncomplete")
          for (const fn of broadcasts) fn();
      });
    } catch {
      clear();
      emit();
    }
  }
  function prepare() {
    if (disposed || queued || client.state() !== "ready" || !publicDocument())
      return;
    queued = true;
    try {
      google.googlefc ??= { callbackQueue: [] };
      google.googlefc.callbackQueue.push({
        CONSENT_API_READY: () => {
          if (disposed) return;
          ready = true;
          attach();
          if (browser.location?.hash === "#manage") {
            try {
              google.googlefc?.showRevocationMessage?.();
            } catch {
              clear();
              emit();
            }
          }
        },
      });
    } catch {
      clear();
      emit();
    }
  }
  const removers = [
    client.subscribe(prepare),
    context.subscribe(() => {
      if (ready) attach();
      else {
        clear();
        emit();
      }
    }),
  ];
  prepare();
  const current = () =>
    !disposed &&
    receipt &&
    receipt.context === context.capture() &&
    Date.now() >= receipt.validFrom &&
    Date.now() < receipt.expiresAt;
  return {
    read: () => (current() ? receipt : { status: "unknown" }),
    evidence: () => (current() ? evidence : undefined),
    subscribe(fn) {
      if (!disposed) listeners.add(fn);
      return () => {
        listeners.delete(fn);
      };
    },
    subscribeBroadcast(fn) {
      if (!disposed) broadcasts.add(fn);
      return () => {
        broadcasts.delete(fn);
      };
    },
    async reopen(signal) {
      if (disposed || signal.aborted) throw Error("unavailable");
      if (
        !publicDocument() ||
        !browser.location.pathname.endsWith("/consent")
      ) {
        navigateToChoices();
        return;
      }
      if (
        !ready ||
        typeof google.googlefc?.showRevocationMessage !== "function"
      )
        throw Error("unavailable");
      clear();
      emit();
      google.googlefc.showRevocationMessage();
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      detach();
      clear();
      for (const remove of removers) remove();
      listeners.clear();
      broadcasts.clear();
    },
  };
}
