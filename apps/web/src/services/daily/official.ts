import type { DailyCore } from "@liar/core-bridge";
import {
  DAILY_SEED_VERSION,
  type DailyCompletion,
  type DailyDisposition,
  type DailyReplay,
  type DailyStartResponse,
  type DailyTrust,
  type DailyView,
  type PublicAction,
  type RulesSnapshot,
} from "@liar/protocol";
import type { AuthPort } from "../auth/types";
import type { RequestOwner } from "../auth/requests";
import type { ActivityPort } from "../activity";
import {
  dailyId,
  type DailyRecord,
  type DailyRecordsPort,
} from "../daily-records";
import { utcDailyDate } from "../daily-session";
import { LocalController, type SessionClock } from "../local-session";
import { sameMetadata } from "./decode";
import type { DailyApi } from "./types";
export type DailyChallengeApi = Pick<
  DailyApi,
  "start" | "heartbeat" | "submitTimed" | "definition" | "own"
>;
export interface OfficialDailyState {
  generation: number;
  status:
    | "idle"
    | "loading"
    | "playing"
    | "saving"
    | "submitting"
    | "pending"
    | "result"
    | "empty"
    | "error"
    | "interrupted"
    | "incomplete";
  view: DailyView | null;
  receipt: DailyCompletion | null;
  trust: DailyTrust | null;
  disposition: DailyDisposition | null;
  rules: RulesSnapshot | null;
  timing: boolean;
  warning: boolean;
}
const initial = (): OfficialDailyState => ({
  generation: 0,
  status: "idle",
  view: null,
  receipt: null,
  trust: null,
  disposition: null,
  rules: null,
  timing: false,
  warning: false,
});
interface Dependencies {
  auth: Pick<AuthPort, "account" | "revision" | "connected" | "subscribe">;
  api: DailyChallengeApi;
  core(): Promise<DailyCore>;
  records: DailyRecordsPort;
  activity: ActivityPort;
  clock: SessionClock;
  wallClock(): number;
}
export class OfficialDailyController {
  private state = initial();
  private listeners = new Set<() => void>();
  private owner: RequestOwner | null = null;
  private request: AbortController | null = null;
  private run: LocalController<DailyView> | null = null;
  private captureReplay: (() => Promise<DailyReplay>) | null = null;
  private attempt = "";
  private grant: DailyStartResponse | null = null;
  private record: DailyRecord | null = null;
  private sequence = 0;
  private beat: Promise<void> | null = null;
  private beatRequest: AbortController | null = null;
  private stopBeat?: () => void;
  private stopRun?: () => void;
  private release?: () => void;
  private stopAuth?: () => void;
  private online = true;
  private disposed = false;
  private generation = 0;
  constructor(private deps: Dependencies) {}
  activate() {
    if (this.disposed || this.stopAuth) return;
    this.stopAuth = this.deps.auth.subscribe(() => {
      if (this.owner && !this.current()) this.interrupt();
    });
  }
  read() {
    return this.state;
  }
  subscribe(listener: () => void) {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }
  private publish(patch: Partial<OfficialDailyState>) {
    if (this.disposed) return;
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) listener();
  }
  private current(owner = this.owner, request = this.request) {
    return (
      !this.disposed &&
      this.online &&
      owner !== null &&
      this.owner === owner &&
      request !== null &&
      this.request === request &&
      !request.signal.aborted &&
      this.deps.auth.connected() &&
      this.deps.auth.account() === owner.accountId &&
      this.deps.auth.revision() === owner.revision
    );
  }
  private busy() {
    return ["loading", "playing", "saving", "submitting", "pending"].includes(
      this.state.status,
    );
  }
  private stop() {
    this.request?.abort();
    this.request = null;
    this.stopBeat?.();
    this.stopBeat = undefined;
    this.beatRequest?.abort();
    this.beatRequest = null;
    this.stopRun?.();
    this.stopRun = undefined;
    this.run?.dispose();
    this.run = null;
    this.captureReplay = null;
    this.release?.();
    this.release = undefined;
  }
  private interrupt() {
    this.stop();
    this.owner = null;
    this.record = null;
    this.attempt = "";
    this.publish({ ...initial(), status: "interrupted" });
  }
  setOnline(online: boolean) {
    this.online = online;
    if (!online && this.owner) this.interrupt();
  }
  private prepare(owner: RequestOwner) {
    this.stop();
    this.owner = owner;
    this.request = new AbortController();
    this.record = null;
    this.grant = null;
    this.beat = null;
    this.sequence = 0;
    this.release = this.deps.activity.hold();
    this.publish({
      ...initial(),
      generation: ++this.generation,
      status: "loading",
    });
    return this.request;
  }
  async start(confirmed: RequestOwner) {
    if (
      this.disposed ||
      this.busy() ||
      !this.online ||
      !this.deps.auth.connected() ||
      this.deps.auth.account() !== confirmed.accountId ||
      this.deps.auth.revision() !== confirmed.revision
    )
      return;
    const retry =
      this.state.status === "error" &&
      this.owner?.accountId === confirmed.accountId &&
      this.owner.revision === confirmed.revision
        ? this.attempt
        : "";
    const owner = { ...confirmed },
      request = this.prepare(owner);
    this.attempt = retry || crypto.randomUUID();
    try {
      const grant = await this.deps.api.start(
        owner,
        this.attempt,
        request.signal,
      );
      if (!this.current(owner, request)) return;
      if (
        grant.heartbeat_interval_ms > 2147483647 ||
        grant.heartbeat_interval_ms < 1
      )
        throw Error("invalid_interval");
      this.grant = grant;
      this.publish({ rules: grant.rules, timing: true });
      this.stopBeat = this.deps.clock.repeat(
        () => this.heartbeat(),
        grant.heartbeat_interval_ms,
      );
      const run = new LocalController<DailyView>(
        async () => {
          const core = await this.deps.core();
          if (!this.current(owner, request)) {
            core.dispose();
            throw Error("cancelled");
          }
          let replay: Promise<DailyReplay> | undefined;
          this.captureReplay = () => (replay ??= core.replay());
          return {
            init: async () => {
              const view = await core.init(
                grant.metadata.date,
                grant.metadata.seed_version,
                grant.rules,
              );
              if (
                !sameMetadata(view.metadata, grant.metadata) ||
                view.rules.hash !== grant.rules.hash
              )
                throw Error("metadata_mismatch");
              return view;
            },
            advance: (time) => core.advance(time),
            step: (input, time) => core.step(input, time),
            dispose: () => {
              if (replay)
                void replay.catch(() => {}).finally(() => core.dispose());
              else core.dispose();
            },
          };
        },
        this.deps.clock,
        (view) => ["finished", "aborted", "cancelled"].includes(view.phase),
      );
      this.run = run;
      this.stopRun = run.subscribe(() => {
        if (!this.current(owner, request)) return;
        const local = run.read();
        if (local.status === "error") {
          this.attempt = "";
          this.stopBeat?.();
          this.stopBeat = undefined;
          this.release?.();
          this.release = undefined;
          this.publish({ status: "error", view: null, timing: false });
        } else if (local.view) {
          this.publish({ view: local.view });
          if (
            local.view.completed &&
            this.state.status !== "saving" &&
            this.state.status !== "submitting" &&
            this.state.status !== "result" &&
            this.state.status !== "pending"
          )
            void this.complete(local.view, owner, request, this.attempt);
          else if (
            ["finished", "aborted", "cancelled"].includes(local.view.phase) &&
            !local.view.completed
          ) {
            this.stopBeat?.();
            this.stopBeat = undefined;
            this.release?.();
            this.release = undefined;
            this.publish({ status: "incomplete", timing: false });
          } else if (this.state.status === "loading")
            this.publish({ status: "playing" });
        }
      });
      await run.start();
    } catch {
      if (this.current(owner, request)) {
        if (this.grant) this.attempt = "";
        this.stopBeat?.();
        this.stopBeat = undefined;
        this.release?.();
        this.release = undefined;
        this.publish({ status: "error", timing: false });
      }
    }
  }
  private loseTiming() {
    this.stopBeat?.();
    this.stopBeat = undefined;
    this.publish({ timing: false });
  }
  private heartbeat() {
    if (this.beat || !this.current() || !this.grant || !this.state.timing)
      return;
    const owner = this.owner!,
      parent = this.request!,
      grant = this.grant;
    const request = new AbortController();
    this.beatRequest = request;
    const sequence = ++this.sequence;
    this.beat = (async () => {
      try {
        const result = await this.deps.api.heartbeat(
          owner,
          grant.ticket_id,
          sequence,
          request.signal,
        );
        if (
          this.current(owner, parent) &&
          this.beatRequest === request &&
          result.ack === null
        )
          this.loseTiming();
      } catch {
        if (this.current(owner, parent) && this.beatRequest === request)
          this.loseTiming();
      } finally {
        if (this.beatRequest === request) {
          this.beatRequest = null;
          this.beat = null;
        }
      }
    })();
  }
  private async complete(
    view: DailyView,
    owner: RequestOwner,
    request: AbortController,
    attempt: string,
  ) {
    const release = this.release;
    this.stopBeat?.();
    this.stopBeat = undefined;
    this.publish({ status: "saving" });
    try {
      const replay = await this.captureReplay!();
      if (!sameMetadata(replay.metadata, view.metadata))
        throw Error("metadata_mismatch");
      const record: DailyRecord = {
        v: 1,
        id: dailyId(view.metadata),
        status: "unverified",
        attempt_id: attempt,
        metadata: view.metadata,
        elapsed_ms: view.elapsed_ms,
        mistakes: view.own.stats.mistakes,
        replay,
      };
      if (this.current(owner, request)) this.record = record;
      let warning = true;
      try {
        warning = (await this.deps.records.saveFirst(record)).warning;
      } catch {
        /* Keep the in-memory candidate for explicit retry. */
      }
      if (!this.current(owner, request)) return;
      this.publish({ warning });
      await this.beat;
      if (this.current(owner, request)) await this.send();
    } catch {
      if (this.current(owner, request))
        this.publish({ status: "error", warning: true });
    } finally {
      release?.();
      if (this.release === release) this.release = undefined;
    }
  }
  private async send() {
    if (!this.current() || !this.record || !this.grant) return;
    const owner = this.owner!,
      request = this.request!,
      record = this.record;
    this.publish({ status: "submitting" });
    try {
      const reply = await this.deps.api.submitTimed(
        owner,
        record.attempt_id,
        this.state.timing ? this.grant.ticket_id : null,
        record.replay,
        request.signal,
      );
      if (!this.current(owner, request)) return;
      let warning = this.state.warning;
      try {
        const removed = await this.deps.records.removePending(
          record.id,
          record.attempt_id,
        );
        warning ||= removed.warning;
      } catch {
        warning = true;
      }
      if (this.current(owner, request))
        this.publish({
          status: "result",
          receipt: reply.receipt,
          trust: reply.trust,
          disposition: reply.disposition,
          warning,
        });
    } catch {
      if (this.current(owner, request)) this.publish({ status: "pending" });
    }
  }
  async retry() {
    if (this.state.status !== "pending" || !this.current()) return;
    const release = this.deps.activity.hold();
    try {
      await this.send();
    } finally {
      release();
    }
  }
  async inspect() {
    if (
      this.disposed ||
      this.busy() ||
      !this.online ||
      !this.deps.auth.connected()
    )
      return;
    const accountId = this.deps.auth.account();
    if (!accountId) return;
    const owner = { accountId, revision: this.deps.auth.revision() },
      request = this.prepare(owner);
    const release = this.release;
    this.attempt = "";
    try {
      const date = utcDailyDate(this.deps.wallClock());
      if (!date) throw Error("invalid_time");
      const definition = await this.deps.api.definition(
        owner,
        date,
        DAILY_SEED_VERSION,
        request.signal,
      );
      if (!this.current(owner, request)) return;
      const own = await this.deps.api.own(
        owner,
        definition.metadata,
        request.signal,
      );
      if (this.current(owner, request))
        this.publish({
          status: own.receipt === null ? "empty" : "result",
          receipt: own.receipt,
          trust: own.trust,
          rules: definition.rules,
        });
    } catch {
      if (this.current(owner, request)) this.publish({ status: "error" });
    } finally {
      release?.();
      if (this.release === release) this.release = undefined;
    }
  }
  submit(action: PublicAction) {
    return this.current() && this.state.status === "playing"
      ? this.run!.submit(action)
      : Promise.resolve();
  }
  result() {
    return this.current() &&
      this.state.receipt &&
      this.state.trust &&
      this.state.rules
      ? {
          receipt: this.state.receipt,
          trust: this.state.trust,
          rules: this.state.rules,
        }
      : null;
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.stop();
    this.stopAuth?.();
    this.stopAuth = undefined;
    this.owner = null;
    this.record = null;
    this.grant = null;
    this.state = { ...initial(), status: "interrupted" };
    this.listeners.clear();
  }
}
