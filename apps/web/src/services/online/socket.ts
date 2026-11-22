import type { AuthenticatedRequests } from "../auth/requests";
import { AuthError } from "../auth/types";
import {
  MAX_INPUT_BYTES,
  MAX_OUTPUT_BYTES,
  type OnlineEvent,
  type OnlineInput,
} from "@liar/protocol";
import { decodeEvent } from "./decode";
import { OnlineFailure, type OnlineConnection, type OnlinePort } from "./types";
import type { RequestOwner } from "../auth/requests";
export type SocketFactory = (url: string) => WebSocket;
export function createOnlineSocket(
  auth: AuthenticatedRequests,
  origin: string,
  factory: SocketFactory,
): OnlinePort["connect"] {
  return (owner, matchId, signal) =>
    connectSocket<OnlineInput, OnlineEvent>(
      auth,
      origin,
      factory,
      owner,
      signal,
      {
        path: "/api/v1/ws",
        decode(raw) {
          const decoded = decodeEvent(raw);
          if (decoded.match_id !== matchId) throw Error();
          return decoded;
        },
        initial: (event) => event.payload.type === "snapshot",
        failure: (event) =>
          event.payload.type === "error" ? event.payload.code : undefined,
        payload(input) {
          if (input.match_id !== matchId) return null;
          const {
            v,
            command_id,
            client_seq,
            session_epoch,
            known_revision,
            action: a,
          } = input;
          const action =
            a.type === "attack"
              ? { type: a.type }
              : { type: a.type, cell: a.cell };
          return {
            v,
            match_id: matchId,
            command_id,
            client_seq,
            session_epoch,
            known_revision,
            action,
          };
        },
      },
    );
}

interface SocketContract<Input, Event> {
  path: string;
  decode(raw: unknown): Event;
  initial(event: Event): boolean;
  failure(event: Event): string | undefined;
  payload(input: Input): unknown;
}

export async function connectSocket<Input, Event>(
  auth: AuthenticatedRequests,
  origin: string,
  factory: SocketFactory,
  owner: RequestOwner,
  signal: AbortSignal,
  contract: SocketContract<Input, Event>,
): Promise<OnlineConnection<Input, Event>> {
  let connection: OnlineConnection<Input, Event> | undefined,
    detachProof = () => {},
    terminal: OnlineFailure | undefined;
  try {
    const result = await auth.execute(
      owner,
      async (context) => {
        const base = new URL(origin);
        if (base.protocol !== "https:" || base.origin !== origin)
          throw new OnlineFailure("unavailable");
        const socket = factory(`wss://${base.host}${contract.path}`);
        let closed = false,
          activated = false,
          first = false;
        const buffer: Event[] = [];
        let receive: ((event: Event) => void) | undefined,
          ended: ((error: OnlineFailure) => void) | undefined;
        let accept!: () => void, reject!: (e: Error) => void;
        const initial = new Promise<void>((resolve, fail) => {
          accept = resolve;
          reject = fail;
        });
        function cleanup() {
          socket.removeEventListener("message", message);
          socket.removeEventListener("error", error);
          socket.removeEventListener("close", error);
          context.signal.removeEventListener("abort", abort);
          signal.removeEventListener("abort", abort);
        }
        function close() {
          if (closed) return;
          closed = true;
          cleanup();
          buffer.length = 0;
          socket.close();
        }
        function fail(code: string) {
          if (closed) return;
          terminal = new OnlineFailure(code);
          reject(
            code === "unauthorized" ? new AuthError("auth_required") : terminal,
          );
          close();
          ended?.(terminal);
        }
        function abort() {
          fail("cancelled");
        }
        function error(event: globalThis.Event) {
          fail(
            event.type === "close" && (event as CloseEvent).code === 1008
              ? "unauthorized"
              : "disconnected",
          );
        }
        function message(event: MessageEvent) {
          if (closed) return;
          try {
            if (
              typeof event.data !== "string" ||
              new TextEncoder().encode(event.data).byteLength > MAX_OUTPUT_BYTES
            )
              throw Error();
            const decoded = contract.decode(JSON.parse(event.data));
            if (!first) {
              const errorCode = contract.failure(decoded);
              if (errorCode) {
                fail(errorCode);
                return;
              }
              if (!contract.initial(decoded)) throw Error();
              first = true;
            }
            if (activated) receive?.(decoded);
            else {
              if (buffer.length >= 16) throw Error();
              buffer.push(decoded);
              accept();
            }
          } catch {
            fail("malformed");
          }
        }
        socket.addEventListener("message", message);
        socket.addEventListener("error", error);
        socket.addEventListener("close", error);
        context.signal.addEventListener("abort", abort, { once: true });
        signal.addEventListener("abort", abort, { once: true });
        detachProof = () => context.signal.removeEventListener("abort", abort);
        connection = {
          listen(event, end) {
            if (activated) throw new OnlineFailure("conflict");
            if (closed) {
              end(terminal ?? new OnlineFailure("disconnected"));
              return;
            }
            activated = true;
            receive = event;
            ended = end;
            for (const item of buffer.splice(0)) {
              if (closed) break;
              event(item);
            }
          },
          send(input) {
            if (closed || !activated || socket.readyState !== 1) return false;
            const payload = contract.payload(input);
            if (payload === null) return false;
            const wire = JSON.stringify(payload);
            if (new TextEncoder().encode(wire).byteLength > MAX_INPUT_BYTES)
              return false;
            try {
              socket.send(wire);
              return true;
            } catch {
              fail("disconnected");
              return false;
            }
          },
          close,
        };
        if (signal.aborted || context.signal.aborted) abort();
        await initial;
        return connection;
      },
      signal,
    );
    detachProof();
    if (terminal) throw terminal;
    return result;
  } catch (error) {
    detachProof();
    connection?.close();
    throw error;
  }
}
