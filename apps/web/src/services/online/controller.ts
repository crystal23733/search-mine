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
  type OnlineInput,
} from "@liar/protocol";
import type { AuthPort } from "../auth/types";
import type { RequestOwner } from "../auth/requests";
import { OnlineFailure, type OnlineConnection, type OnlinePort } from "./types";
import { roomCode } from "./invitation";
import { BoundedReconnect } from "./reconnect";
export interface OnlineState {
  status:
    | "loading"
    | "ready"
    | "waiting"
    | "connecting"
    | "reconnecting"
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
  reconnectMs: number | null;
}
export type OnlineAuth = Pick<
  AuthPort,
  | "account"
  | "revision"
  | "connected"
  | "subscribe"
  | "invalidate"
  | "execute"
  | "recoveryOwner"
  | "resume"
>;
interface PendingCommand {
  input: OnlineInput;
  timer?: ReturnType<typeof setTimeout>;
  replayed: boolean;
}
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
    reconnectMs: null,
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
  private commands = new Map<string, PendingCommand>();
  private lease = 0;
  private recovery: BoundedReconnect;
  private listeners = new Set<() => void>();
  constructor(
    private auth: OnlineAuth,
    private port: OnlinePort,
    private now: () => number = () => performance.now(),
    random: () => number = () => Math.random(),
  ) {
    this.recovery = new BoundedReconnect(
      now,
      random,
      (signal) => this.reconnect(signal),
      () => this.fail("timeout"),
    );
  }
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
              reconnect_ms:
                view.opponent.reconnect_ms === null
                  ? null
                  : Math.max(0, view.opponent.reconnect_ms - elapsed),
            },
          }
        : view;
    const lobby = this.state.lobby;
    return {
      ...this.state,
      view: shown,
      pending: this.commands.size,
      reconnectMs: this.recovery.remaining(),
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
  private owned() {
    if (this.disposed || !this.owner) return false;
    if (this.current(this.generation)) return true;
    const candidate = this.auth.recoveryOwner();
    return (
      candidate?.accountId === this.owner.accountId &&
      candidate.revision === this.owner.revision
    );
  }
  private closeConnection() {
    this.lease++;
    const connection = this.connection;
    this.connection = null;
    connection?.close();
  }
  private clearCommands() {
    for (const command of this.commands.values()) clearTimeout(command.timer);
    this.commands.clear();
  }
  private stop() {
    this.generation++;
    this.recovery.stop();
    this.abort.abort();
    this.abort = new AbortController();
    clearTimeout(this.poll);
    this.poll = undefined;
    this.closeConnection();
    this.clearCommands();
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
        if (!this.owned()) this.fail("unauthorized", true);
        else if (!this.auth.connected()) this.beginRecovery();
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
        const lease = ++this.lease;
        connection.listen(
          (event) => {
            if (this.current(generation) && lease === this.lease)
              this.receive(event);
          },
          (error) => {
            if (
              this.current(generation) &&
              lease === this.lease &&
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
    if (match?.type !== "matched" || event.match_id !== match.match_id) {
      this.fail("stale");
      return;
    }
    if (this.seq !== 0 && event.server_seq !== this.seq + 1) {
      this.beginRecovery();
      return;
    }
    const first = this.seq === 0;
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
      if (this.state.view?.result) return;
      const command = this.commands.get(payload.command_id);
      if (!command) {
        this.fail("stale");
        return;
      }
      clearTimeout(command.timer);
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
      if (!first || (this.epoch !== 0 && payload.session_epoch <= this.epoch)) {
        this.fail("stale");
        return;
      }
      this.epoch = payload.session_epoch;
      this.clientSeq = Math.max(this.clientSeq, payload.last_client_seq);
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
    if (view.result) this.clearCommands();
    this.publish();
  }
  private beginRecovery() {
    if (!this.owned()) {
      this.fail("unauthorized", true);
      return;
    }
    if (this.state.lobby?.state.type !== "matched" || this.state.view?.result) {
      this.fail("disconnected");
      return;
    }
    if (this.recovery.remaining() !== null) return;
    this.generation++;
    this.abort.abort();
    this.abort = new AbortController();
    clearTimeout(this.poll);
    this.poll = undefined;
    this.closeConnection();
    for (const command of this.commands.values()) {
      clearTimeout(command.timer);
      command.timer = undefined;
    }
    this.seq = 0;
    this.state = {
      ...this.state,
      status: "reconnecting",
      working: false,
      error: null,
    };
    this.recovery.start();
    this.publish();
  }
  private async reconnect(signal: AbortSignal): Promise<boolean> {
    const generation = this.generation,
      owner = this.owner,
      match = this.state.lobby?.state;
    if (!owner || match?.type !== "matched" || !this.owned()) {
      this.fail("unauthorized", true);
      return false;
    }
    const current = () =>
      generation === this.generation &&
      !signal.aborted &&
      this.owned() &&
      (this.recovery.remaining() ?? 0) > 0;
    if (!this.auth.connected()) {
      await this.auth.resume(owner);
      if (!current()) return false;
      if (!this.auth.connected()) return false;
    }
    const lease = ++this.lease;
    try {
      const connection = await this.port.connect(
        owner,
        match.match_id,
        AbortSignal.any([signal, this.abort.signal]),
      );
      if (!current() || lease !== this.lease || !this.auth.connected()) {
        connection.close();
        return false;
      }
      this.connection = connection;
      this.seq = 0;
      connection.listen(
        (event) => {
          if (
            this.current(generation) &&
            lease === this.lease &&
            !signal.aborted &&
            this.recovery.remaining() !== 0
          )
            this.receive(event);
        },
        (error) => {
          if (
            this.current(generation) &&
            lease === this.lease &&
            !signal.aborted &&
            this.recovery.remaining() !== 0 &&
            this.state.recording !== "saved" &&
            this.state.recording !== "failed"
          )
            this.disconnected(error.code);
        },
      );
      if (!current() || lease !== this.lease) {
        if (lease === this.lease && this.connection === connection)
          this.closeConnection();
        else connection.close();
        return false;
      }
      if (this.state.status !== "playing" && this.state.status !== "ended") {
        this.closeConnection();
        return false;
      }
      for (const command of this.commands.values()) {
        if (command.input.session_epoch === this.epoch) continue;
        command.input = { ...command.input, session_epoch: this.epoch };
        command.replayed = true;
        this.arm(command);
        if (!connection.send(command.input)) {
          this.closeConnection();
          this.state = { ...this.state, status: "reconnecting" };
          this.publish();
          return false;
        }
      }
      return true;
    } catch (error) {
      if (!current()) return false;
      const code =
        error instanceof Error &&
        "code" in error &&
        typeof error.code === "string"
          ? error.code
          : "unavailable";
      if (code === "unauthorized" || code === "auth_required")
        this.auth.invalidate();
      else if (
        ![
          "disconnected",
          "unavailable",
          "timeout",
          "auth_unavailable",
          "cancelled",
          "capacity",
        ].includes(code)
      )
        this.fail(code);
      return false;
    }
  }
  private arm(command: PendingCommand) {
    clearTimeout(command.timer);
    const generation = this.generation;
    command.timer = setTimeout(() => {
      if (!this.current(generation)) return;
      if (command.replayed) this.fail("timeout");
      else this.beginRecovery();
    }, 10_000);
  }
  private disconnected(code: string) {
    if (code === "unauthorized") {
      this.auth.invalidate();
      return;
    }
    if (code !== "disconnected" && code !== "timeout") {
      this.fail(code);
      return;
    }
    if (this.state.view?.result) {
      this.stop();
      this.state = { ...this.state, status: "ended", error: code };
      this.publish();
    } else this.beginRecovery();
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
    const command: PendingCommand = {
      replayed: false,
      input: {
        v: PROTOCOL_VERSION,
        match_id: match.match_id,
        command_id: commandId,
        client_seq: ++this.clientSeq,
        session_epoch: this.epoch,
        known_revision: view.revision,
        action:
          action.type === "attack"
            ? { type: "attack" }
            : { type: action.type, cell: action.cell },
      },
    };
    this.commands.set(commandId, command);
    this.arm(command);
    if (!this.connection.send(command.input)) {
      if (this.current(generation)) this.beginRecovery();
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
