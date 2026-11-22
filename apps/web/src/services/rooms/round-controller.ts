import {
  ROOM_PROTOCOL_VERSION,
  type PublicAction,
  type RecordingStatus,
  type RoundEvent,
  type RoundView,
} from "@liar/protocol";
import type { RequestOwner } from "../auth/requests";
import type { AuthPort } from "../auth/types";
import { BoundedReconnect } from "../online/reconnect";
import type { RoomPort, RoundConnection } from "./types";

interface Binding {
  roomId: string;
  roundId: string;
  seat: number;
}
interface State {
  status: "connecting" | "ready" | "reconnecting" | "disconnected" | "finished";
  view: RoundView | null;
  receivedAt: number;
  rejected: boolean;
  recording: RecordingStatus | null;
}

const initial: State = {
  status: "connecting",
  view: null,
  receivedAt: 0,
  rejected: false,
  recording: null,
};

export class RoundController {
  private state: State = initial;
  private readonly listeners = new Set<() => void>();
  private connection: RoundConnection | null = null;
  private lifetime: AbortController | null = null;
  private unsubscribe: (() => void) | undefined;
  private readonly reconnect: BoundedReconnect;
  private serverSequence = 0;
  private clientSequence = 0;
  private epoch = 0;

  constructor(
    private auth: AuthPort,
    private port: RoomPort,
    private owner: RequestOwner,
    private binding: Binding,
  ) {
    this.reconnect = new BoundedReconnect(
      () => performance.now(),
      Math.random,
      (signal) => this.open(signal),
      () => this.update({ status: "disconnected" }),
    );
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
  private update(patch: Partial<State>) {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) listener();
  }
  private current() {
    return (
      this.lifetime !== null &&
      this.auth.connected() &&
      this.auth.revision() === this.owner.revision &&
      this.auth.read().account?.id === this.owner.accountId
    );
  }

  start() {
    if (this.lifetime) return;
    this.lifetime = new AbortController();
    this.unsubscribe = this.auth.subscribe(() => {
      if (this.current()) return;
      this.stop();
      this.update({ ...initial, status: "disconnected" });
    });
    void this.open(this.lifetime.signal).then((opened) => {
      if (!opened) this.tryAgain();
    });
  }

  private tryAgain() {
    if (!this.current() || this.state.status === "finished") return;
    this.epoch = 0;
    this.update({ status: "reconnecting" });
    this.reconnect.start();
  }

  private async open(signal: AbortSignal) {
    const lifetime = this.lifetime;
    if (!this.current() || !lifetime) return false;
    try {
      const connection = await this.port.connect(
        this.owner,
        this.binding.roomId,
        this.binding.roundId,
        AbortSignal.any([signal, lifetime.signal]),
      );
      if (!this.current() || this.lifetime !== lifetime || signal.aborted) {
        connection.close();
        return false;
      }
      this.connection?.close();
      this.connection = connection;
      connection.listen(
        (event) => {
          if (this.connection === connection && this.current())
            this.receive(event);
        },
        (error) => {
          if (this.connection !== connection) return;
          this.connection = null;
          if (error.code === "unauthorized") this.auth.invalidate();
          this.tryAgain();
        },
      );
      return this.connection === connection || this.state.status === "finished";
    } catch {
      return false;
    }
  }

  private receive(event: RoundEvent) {
    if (
      event.room_id !== this.binding.roomId ||
      event.round_id !== this.binding.roundId ||
      event.server_seq <= this.serverSequence ||
      this.state.status === "finished"
    )
      return;
    this.serverSequence = event.server_seq;
    const { payload } = event;
    if (payload.type === "error") {
      this.update({ rejected: true });
      return;
    }
    if (payload.type === "ack") {
      this.update({ rejected: payload.status === "rejected" });
      return;
    }
    if (payload.type === "awaiting") return;
    if (
      payload.view.opponents.some(
        (entry) => entry.seat === this.binding.seat,
      ) ||
      (this.state.view && payload.view.revision < this.state.view.revision)
    )
      return;
    if (payload.type === "snapshot") {
      if (payload.own_seat !== this.binding.seat) return;
      this.epoch = payload.session_epoch;
      this.clientSequence = payload.last_client_seq;
    } else if (payload.type === "delta" && this.epoch === 0) return;
    if (payload.type === "end") {
      this.update({
        view: payload.view,
        receivedAt: performance.now(),
        status: "finished",
        recording: payload.recording,
      });
    } else
      this.update({
        view: payload.view,
        receivedAt: performance.now(),
        status: "ready",
      });
  }

  send(action: PublicAction) {
    const { status, view } = this.state;
    if (
      !this.current() ||
      status !== "ready" ||
      view?.phase !== "playing" ||
      view.forfeited ||
      this.epoch === 0
    )
      return;
    const sequence = this.clientSequence + 1;
    if (sequence > 4294967295) return;
    if (
      this.connection?.send({
        v: ROOM_PROTOCOL_VERSION,
        round_id: this.binding.roundId,
        command_id: crypto.randomUUID(),
        client_seq: sequence,
        session_epoch: this.epoch,
        known_revision: view.revision,
        action,
      })
    )
      this.clientSequence = sequence;
    else this.update({ rejected: true });
  }

  private stop() {
    this.lifetime?.abort();
    this.lifetime = null;
    this.reconnect.stop();
    this.connection?.close();
    this.connection = null;
  }

  dispose() {
    this.unsubscribe?.();
    this.unsubscribe = undefined;
    this.stop();
  }
}
