import {
  PROTOCOL_VERSION,
  type GameView,
  type LobbyCommand,
  type LobbyCancellation,
  type LobbyDifficulty,
  type LobbyResponse,
  type PublicAction,
  type RecordingStatus,
  type LobbyOpponent,
  type OnlineEvent,
} from "@liar/protocol";
import type { AuthPort } from "../auth/types";
import type { RequestOwner } from "../auth/requests";
import { OnlineFailure, type OnlineConnection, type OnlinePort } from "./types";
import { roomCode } from "./invitation";
export interface OnlineState {
  status:
    | "loading"
    | "ready"
    | "waiting"
    | "connecting"
    | "playing"
    | "ended"
    | "error";
  lobby: LobbyResponse | null;
  view: GameView | null;
  opponent: LobbyOpponent | null;
  recording: RecordingStatus | null;
  error: string | null;
  working: boolean;
  pending: number;
  waitMs: number;
  roomMs: number;
  difficulty: LobbyDifficulty | null;
}
export type OnlineAuth = Pick<
  AuthPort,
  "account" | "revision" | "connected" | "subscribe" | "invalidate" | "execute"
>;
export class OnlineController {
  private state: OnlineState = {
    status: "loading",
    lobby: null,
    view: null,
    opponent: null,
    recording: null,
    error: null,
    working: false,
    pending: 0,
    waitMs: 0,
    roomMs: 0,
    difficulty: null,
  };
  private owner: RequestOwner | null = null;
  private generation = 0;
  private disposed = false;
  private abort = new AbortController();
  private connection: OnlineConnection | null = null;
  private unsubscribe: (() => void) | null = null;
  private poll: ReturnType<typeof setTimeout> | undefined;
  private ticker: ReturnType<typeof setInterval> | undefined;
  private sampled = 0;
  private seq = 0;
  private epoch = 0;
  private clientSeq = 0;
  private commands = new Map<string, ReturnType<typeof setTimeout>>();
  private listeners = new Set<() => void>();
  constructor(
    private auth: OnlineAuth,
    private port: OnlinePort,
    private now: () => number = () => performance.now(),
  ) {}
  read(): OnlineState {
    const elapsed = Math.max(0, this.now() - this.sampled),
      view = this.state.view;
    const shown =
      view && !view.result
        ? {
            ...view,
            countdown_ms: Math.max(0, view.countdown_ms - elapsed),
            remaining_ms:
              view.phase === "playing"
                ? Math.max(0, view.remaining_ms - elapsed)
                : view.remaining_ms,
            own: {
              ...view.own,
              stun_ms: Math.max(0, view.own.stun_ms - elapsed),
            },
            opponent: {
              ...view.opponent,
              stun_ms: Math.max(0, view.opponent.stun_ms - elapsed),
            },
          }
        : view;
    const lobby = this.state.lobby;
    return {
      ...this.state,
      view: shown,
      pending: this.commands.size,
      roomMs:
        lobby?.state.type === "room"
          ? Math.max(
              0,
              lobby.state.expires_at_ms - lobby.server_time_ms - elapsed,
            )
          : 0,
      waitMs:
        lobby?.state.type === "queued"
          ? Math.max(
              0,
              lobby.state.deadline_ms - lobby.server_time_ms - elapsed,
            )
          : 0,
    };
  }
  subscribe(listener: () => void) {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
  private publish() {
    for (const listener of this.listeners) listener();
  }
  private current(generation: number) {
    return (
      !this.disposed &&
      generation === this.generation &&
      this.owner !== null &&
      this.auth.connected() &&
      this.auth.account() === this.owner.accountId &&
      this.auth.revision() === this.owner.revision
    );
  }
  private stop() {
    this.generation++;
    this.abort.abort();
    this.abort = new AbortController();
    clearTimeout(this.poll);
    this.poll = undefined;
    this.connection?.close();
    this.connection = null;
    for (const timer of this.commands.values()) clearTimeout(timer);
    this.commands.clear();
    this.seq = 0;
    this.epoch = 0;
    this.clientSeq = 0;
  }
  private fail(code: string, clear = false) {
    this.stop();
    this.state = {
      ...this.state,
      status: "error",
      working: false,
      error: code,
      ...(clear
        ? { view: null, lobby: null, recording: null, opponent: null }
        : {}),
    };
    this.publish();
  }
  async start() {
    if (this.disposed) return;
    this.stop();
    const accountId = this.auth.account();
    if (!accountId || !this.auth.connected()) {
      this.fail("unauthorized", true);
      return;
    }
    this.owner = { accountId, revision: this.auth.revision() };
    if (!this.unsubscribe)
      this.unsubscribe = this.auth.subscribe(() => {
        if (!this.current(this.generation)) this.fail("unauthorized", true);
      });
    if (!this.ticker)
      this.ticker = setInterval(() => {
        if (
          (this.state.view && !this.state.view.result) ||
          this.state.status === "waiting"
        )
          this.publish();
      }, 250);
    this.state = {
      ...this.state,
      status: "loading",
      lobby: null,
      view: null,
      opponent: null,
      recording: null,
      difficulty: null,
      error: null,
      working: true,
    };
    this.publish();
    await this.request({ type: "status" }, this.generation);
  }
  async join(difficulty: LobbyDifficulty) {
    if (this.disposed || this.state.working || this.state.status !== "ready")
      return;
    this.stop();
    this.state = { ...this.state, working: true, error: null, difficulty };
    this.publish();
    await this.request({ type: "queue_join", difficulty }, this.generation);
  }
  private async intent(command: LobbyCommand) {
    this.stop();
    this.state = { ...this.state, working: true, error: null };
    this.publish();
    await this.request(command, this.generation);
  }
  async createRoom() {
    if (this.disposed || this.state.working || this.state.status !== "ready")
      return;
    await this.intent({ type: "room_create" });
  }
  async joinRoom(value: string) {
    if (this.disposed || this.state.working || this.state.status !== "ready")
      return;
    const code = roomCode(value);
    if (!code) {
      this.state = { ...this.state, error: "invalid_code" };
      this.publish();
      return;
    }
    await this.intent({ type: "room_join", code });
  }
  async setReady(ready: boolean) {
    const room = this.state.lobby?.state;
    if (
      this.disposed ||
      this.state.working ||
      this.state.status !== "waiting" ||
      room?.type !== "room"
    )
      return;
    await this.intent({ type: "ready", room_id: room.room_id, ready });
  }
  private identity(): LobbyCancellation | null {
    const state = this.state.lobby?.state;
    return state?.type === "queued"
      ? { kind: "queue", queue_id: state.queue_id }
      : state?.type === "room"
        ? { kind: "room", room_id: state.room_id }
        : state?.type === "preparing"
          ? state.identity
          : null;
  }
  async cancel() {
    const identity = this.identity();
    if (!identity || this.disposed || this.state.working) return;
    this.stop();
    this.state = { ...this.state, working: true, error: null };
    this.publish();
    await this.request({ type: "cancel", identity }, this.generation);
  }
  private async request(command: LobbyCommand, generation: number) {
    if (!this.current(generation) || !this.owner) return;
    try {
      const response = await this.port.lobby(
        this.owner,
        command,
        this.abort.signal,
      );
      if (!this.current(generation)) return;
      if (response.state.type === "failed")
        throw new OnlineFailure(response.state.error);
      if (command.type === "cancel" && response.state.type !== "idle")
        throw new OnlineFailure("busy");
      if (response.state.type === "queued")
        this.state = { ...this.state, difficulty: response.state.difficulty };
      this.sampled = this.now();
      this.state = {
        ...this.state,
        lobby: response,
        working: false,
        error: null,
      };
      if (response.state.type === "matched") {
        this.state = {
          ...this.state,
          status: "connecting",
          opponent: response.state.opponent,
        };
        this.publish();
        const connection = await this.port.connect(
          this.owner,
          response.state.match_id,
          this.abort.signal,
        );
        if (!this.current(generation)) {
          connection.close();
          return;
        }
        this.connection = connection;
        connection.listen(
          (event) => {
            if (this.current(generation)) this.receive(event);
          },
          (error) => {
            if (
              this.current(generation) &&
              this.state.recording !== "saved" &&
              this.state.recording !== "failed"
            )
              this.disconnected(error.code);
          },
        );
      } else {
        this.state = {
          ...this.state,
          status: response.state.type === "idle" ? "ready" : "waiting",
        };
        this.publish();
        if (this.state.status === "waiting")
          this.poll = setTimeout(() => {
            this.poll = undefined;
            void this.request({ type: "status" }, generation);
          }, 500);
      }
    } catch (error) {
      if (this.current(generation)) {
        const code =
          error instanceof Error &&
          "code" in error &&
          typeof error.code === "string"
            ? error.code
            : "unavailable";
        if (code === "unauthorized" || code === "auth_required")
          this.auth.invalidate();
        else this.fail(code);
      }
    }
  }
  private receive(event: OnlineEvent) {
    const match = this.state.lobby?.state;
    if (
      match?.type !== "matched" ||
      event.match_id !== match.match_id ||
      (this.seq !== 0 && event.server_seq !== this.seq + 1)
    ) {
      this.fail("stale");
      return;
    }
    if (this.seq === 0 && event.payload.type !== "snapshot") {
      this.fail("stale");
      return;
    }
    this.seq = event.server_seq;
    const payload = event.payload;
    if (payload.type === "error") {
      if (payload.code === "unauthorized") {
        this.auth.invalidate();
        return;
      }
      this.fail(payload.code);
      return;
    }
    if (payload.type === "ack") {
      const timer = this.commands.get(payload.command_id);
      if (timer === undefined) {
        this.fail("stale");
        return;
      }
      clearTimeout(timer);
      this.commands.delete(payload.command_id);
      this.state = {
        ...this.state,
        error:
          payload.status === "rejected"
            ? `input_${payload.error ?? "unavailable"}`
            : null,
      };
      this.publish();
      return;
    }
    if (payload.type === "snapshot") {
      if (this.epoch !== 0 && this.epoch !== payload.session_epoch) {
        this.fail("stale");
        return;
      }
      this.epoch = payload.session_epoch;
    }
    const view = payload.view,
      previous = this.state.view;
    if (
      previous &&
      (view.rules.hash !== previous.rules.hash ||
        view.revision < previous.revision)
    ) {
      this.fail("stale");
      return;
    }
    this.sampled = this.now();
    this.state = {
      ...this.state,
      view,
      status: view.result ? "ended" : "playing",
      ...(payload.type === "match_end" ? { recording: payload.recording } : {}),
    };
    this.publish();
  }
  private disconnected(code: string) {
    this.fail(code);
    const owner = this.owner;
    if (owner && !this.disposed) {
      // A revoked server session can arrive as a close frame without a public error DTO.
      void this.auth
        .execute(owner, async () => undefined, this.abort.signal)
        .catch(() => {});
    }
  }
  submit(action: PublicAction) {
    const view = this.state.view,
      match = this.state.lobby?.state;
    if (
      this.disposed ||
      this.state.status !== "playing" ||
      view?.phase !== "playing" ||
      match?.type !== "matched" ||
      !this.connection ||
      !this.current(this.generation)
    )
      return;
    if (this.commands.size >= 16) {
      this.state = { ...this.state, error: "capacity" };
      this.publish();
      return;
    }
    if (this.clientSeq >= 4294967295 || view.revision > 4294967295) {
      this.fail("capacity");
      return;
    }
    const commandId = crypto.randomUUID(),
      generation = this.generation;
    this.commands.set(
      commandId,
      setTimeout(() => {
        if (this.current(generation)) this.fail("timeout");
      }, 10000),
    );
    if (
      !this.connection.send({
        v: PROTOCOL_VERSION,
        match_id: match.match_id,
        command_id: commandId,
        client_seq: ++this.clientSeq,
        session_epoch: this.epoch,
        known_revision: view.revision,
        action,
      })
    ) {
      if (this.current(generation)) this.fail("disconnected");
      return;
    }
    this.state = { ...this.state, error: null };
    this.publish();
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.stop();
    this.unsubscribe?.();
    this.unsubscribe = null;
    clearInterval(this.ticker);
    this.ticker = undefined;
    this.listeners.clear();
    this.owner = null;
    this.state = {
      ...this.state,
      view: null,
      lobby: null,
      recording: null,
      opponent: null,
    };
  }
}
