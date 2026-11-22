import type {
  RoomList,
  RoomListCursor,
  RoomView,
  RoomWireError,
} from "@liar/protocol";
import type { RequestOwner } from "../auth/requests";
import type { AuthPort } from "../auth/types";
import { RoomFailure, type RoomCommand, type RoomPort } from "./types";

interface State {
  page: RoomList | null;
  cursor: RoomListCursor | null;
  room: RoomView | null;
  pending: boolean;
  listError: boolean;
  error: RoomWireError | null;
  retry: RoomCommand | null;
}

export class RoomsController {
  private state: State = {
    page: null,
    cursor: null,
    room: null,
    pending: false,
    listError: false,
    error: null,
    retry: null,
  };
  private readonly listeners = new Set<() => void>();
  private ownerKey = "";
  private roomRead: AbortController | null = null;
  private listRead: AbortController | null = null;
  private action: AbortController | null = null;
  private cleanup: (() => void) | null = null;

  constructor(
    private auth: AuthPort,
    private port: RoomPort,
    private available: () => boolean,
  ) {}

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
  private owner(): RequestOwner | null {
    const account = this.auth.read().account;
    return account?.nickname && this.auth.connected()
      ? { accountId: account.id, revision: this.auth.revision() }
      : null;
  }
  private cancelStatus() {
    this.roomRead?.abort();
    this.roomRead = null;
  }

  start() {
    if (this.cleanup) return;
    const authorityChanged = () => {
      const owner = this.owner();
      const key = owner ? `${owner.accountId}:${owner.revision}` : "";
      if (key === this.ownerKey) return;
      this.ownerKey = key;
      this.cancelStatus();
      this.action?.abort();
      this.action = null;
      this.update({ room: null, pending: false, retry: null, error: null });
      void this.status();
    };
    const stopAuthority = this.auth.subscribe(authorityChanged);
    const roomTick = setInterval(() => void this.status(), 1000);
    const listTick = setInterval(() => void this.list(), 5000);
    this.cleanup = () => {
      stopAuthority();
      clearInterval(roomTick);
      clearInterval(listTick);
    };
    authorityChanged();
    void this.list();
  }

  async list(cursor = this.state.cursor) {
    if (!this.cleanup || this.listRead || !this.available()) return;
    const attempt = new AbortController();
    this.listRead = attempt;
    try {
      const page = await this.port.list(cursor, attempt.signal);
      if (!attempt.signal.aborted)
        this.update({ page, cursor, listError: false });
    } catch {
      if (!attempt.signal.aborted) this.update({ listError: true });
    } finally {
      if (this.listRead === attempt) this.listRead = null;
    }
  }

  private async status() {
    const owner = this.owner();
    if (
      !owner ||
      !this.cleanup ||
      this.roomRead ||
      this.action ||
      !this.available()
    )
      return;
    const attempt = new AbortController();
    this.roomRead = attempt;
    try {
      const room = await this.port.command(
        owner,
        { type: "status" },
        attempt.signal,
      );
      if (!attempt.signal.aborted) this.accept(room);
    } catch {
      if (!attempt.signal.aborted && !this.state.error)
        this.update({ error: "unavailable" });
    } finally {
      if (this.roomRead === attempt) this.roomRead = null;
    }
  }

  private accept(room: RoomView | null) {
    const current = this.state.room;
    if (
      room &&
      current?.room_id === room.room_id &&
      BigInt(room.revision) < BigInt(current.revision)
    )
      return;
    this.update({ room });
  }

  async mutate(intent: RoomCommand) {
    const owner = this.owner();
    if (
      !owner ||
      !this.cleanup ||
      this.action ||
      !this.available() ||
      intent.type === "status"
    )
      return false;
    this.cancelStatus();
    const attempt = new AbortController();
    this.action = attempt;
    this.update({ pending: true, error: null, retry: null });
    try {
      const room = await this.port.command(owner, intent, attempt.signal);
      if (!attempt.signal.aborted) {
        this.accept(room);
        return true;
      }
    } catch (cause) {
      if (attempt.signal.aborted) return false;
      const error = cause instanceof RoomFailure ? cause.code : "unavailable";
      this.update({ error, retry: error === "unavailable" ? intent : null });
    } finally {
      if (this.action === attempt) {
        this.action = null;
        this.update({ pending: false });
        void this.status();
      }
    }
    return false;
  }

  dispose() {
    this.cleanup?.();
    this.cleanup = null;
    this.ownerKey = "";
    this.cancelStatus();
    this.action?.abort();
    this.listRead?.abort();
    this.action = this.listRead = null;
  }
}
