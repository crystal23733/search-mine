import {
  OPTIONAL_EVENT_PURPOSES,
  type OptionalEventAction,
} from "@liar/protocol";
import type { AuthPort } from "../auth/types";
import type { ConsentPort, ConsentView } from "../consent";
import type { Locale } from "../locale";
import type {
  ConsentControlPort,
  EventTransport,
  OptionalEvents,
} from "./types";
export interface CollectorDependencies {
  auth: AuthPort;
  consent: ConsentPort;
  control: ConsentControlPort;
  transport: EventTransport;
  available(): boolean;
  clock(): { wall: number; monotonic: number };
  id(): string;
}
export function createOptionalEvents(d: CollectorDependencies): OptionalEvents {
  let disposed = false,
    broken = false,
    epoch = 0;
  let active: AbortController | null = null;
  let tab: { account: string; revision: number; key: string } | null = null;
  let previous: ReturnType<CollectorDependencies["clock"]> | null = null;
  const sample = () => {
    const now = d.clock();
    if (
      !Number.isSafeInteger(now.wall) ||
      now.wall < 0 ||
      !Number.isFinite(now.monotonic) ||
      now.monotonic < 0 ||
      (previous &&
        (now.wall < previous.wall || now.monotonic < previous.monotonic))
    )
      broken = true;
    previous = { ...now };
    if (broken) throw Error("clock");
    return now;
  };
  const cancel = () => {
    epoch++;
    tab = null;
    const request = active;
    active = null;
    request?.abort();
  };
  const remove = [d.auth.subscribe(cancel), d.consent.subscribe(cancel)];
  const permitted = (purpose: "analytics" | "advertising", view: ConsentView) =>
    !disposed &&
    !broken &&
    d.available() &&
    view.status === "valid" &&
    !view.working &&
    view[purpose] &&
    d.auth.connected() &&
    !d.auth.read().working &&
    d.auth.read().status === "ready";
  return {
    async record(action: OptionalEventAction<string>, locale: Locale) {
      let accepted = false;
      let controller: AbortController | undefined;
      let timer: ReturnType<typeof setTimeout> | undefined;
      let onAbort: (() => void) | undefined;
      try {
        if (active || !Object.hasOwn(OPTIONAL_EVENT_PURPOSES, action.type))
          return false;
        const purpose = OPTIONAL_EVENT_PURPOSES[action.type],
          view = { ...d.consent.read() };
        if (!permitted(purpose, view)) return false;
        const account = d.auth.account();
        if (!account) return false;
        const owner = { accountId: account, revision: d.auth.revision() },
          generation = epoch;
        const began = sample(),
          occurred_at = Math.floor(began.wall / 1000),
          event = { ...action };
        controller = new AbortController();
        const request = controller;
        active = request;
        const interrupted = new Promise<boolean>((resolve) => {
          onAbort = () => resolve(false);
          request.signal.addEventListener("abort", onAbort, { once: true });
        });
        timer = setTimeout(() => request.abort(), 2000);
        const current = () => {
          const now = d.consent.read();
          return (
            !request.signal.aborted &&
            epoch === generation &&
            permitted(purpose, now) &&
            now.analytics === view.analytics &&
            now.advertising === view.advertising &&
            d.auth.account() === owner.accountId &&
            d.auth.revision() === owner.revision
          );
        };
        const run = async () => {
          const readAt = sample();
          const snapshot = await d.control.read(owner, request.signal);
          const c = snapshot.consent;
          if (
            !current() ||
            snapshot.account_id !== owner.accountId ||
            !snapshot.revision ||
            c.status !== "current" ||
            c[purpose] !== "granted" ||
            c.checked_at === null ||
            c.expires_at === null ||
            c.checked_at > occurred_at
          )
            return false;
          const expires =
            readAt.monotonic + (c.expires_at - snapshot.observed_at) * 1000;
          const ready = sample();
          if (
            ready.monotonic >= expires ||
            ready.monotonic >= began.monotonic + 2000
          )
            return false;
          if (
            !tab ||
            tab.account !== owner.accountId ||
            tab.revision !== owner.revision
          )
            tab = {
              account: owner.accountId,
              revision: owner.revision,
              key: d.id(),
            };
          const id = d.id();
          const value = await d.transport.send(
            owner,
            {
              v: 1,
              expected_revision: snapshot.revision,
              draft: {
                event_version: 1,
                event_id: id,
                session_key: tab.key,
                occurred_at,
                locale,
                event,
              },
            },
            request.signal,
          );
          const completed = sample();
          return (
            current() &&
            completed.monotonic < expires &&
            completed.monotonic < began.monotonic + 2000 &&
            value.v === 1 &&
            value.event_id === id &&
            Number.isSafeInteger(value.received_at) &&
            value.received_at >= occurred_at
          );
        };
        accepted = await Promise.race([run(), interrupted]);
        return accepted;
      } catch {
        return false;
      } finally {
        if (controller && active === controller && !accepted) tab = null;
        clearTimeout(timer);
        if (controller && onAbort)
          controller.signal.removeEventListener("abort", onAbort);
        if (active === controller) active = null;
      }
    },
    cancel,
    dispose() {
      if (disposed) return;
      disposed = true;
      cancel();
      for (const unsubscribe of remove) unsubscribe();
    },
  };
}
