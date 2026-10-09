import type { AuthenticatedRequests } from "../auth/requests";
import { AuthError } from "../auth/types";
import {
  MAX_INPUT_BYTES,
  MAX_OUTPUT_BYTES,
  type OnlineEvent,
} from "@liar/protocol";
import { decodeEvent } from "./decode";
import { OnlineFailure, type OnlineConnection, type OnlinePort } from "./types";
export type SocketFactory = (url: string) => WebSocket;
export function createOnlineSocket(
  auth: AuthenticatedRequests,
  origin: string,
  factory: SocketFactory,
): OnlinePort["connect"] {
  return async (owner, matchId, signal) => {
    let connection: OnlineConnection | undefined,
      detachProof = () => {},
      terminal: OnlineFailure | undefined;
    try {
      const result = await auth.execute(
        owner,
        async (context) => {
          const base = new URL(origin);
          if (base.protocol !== "https:" || base.origin !== origin)
            throw new OnlineFailure("unavailable");
          const socket = factory(`wss://${base.host}/api/v1/ws`);
          let closed = false,
            activated = false,
            first = false;
          const buffer: OnlineEvent[] = [];
          let receive: ((event: OnlineEvent) => void) | undefined,
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
              code === "unauthorized"
                ? new AuthError("auth_required")
                : terminal,
            );
            close();
            ended?.(terminal);
          }
          function abort() {
            fail("cancelled");
          }
          function error(event: Event) {
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
                new TextEncoder().encode(event.data).byteLength >
                  MAX_OUTPUT_BYTES
              )
                throw Error();
              const decoded = decodeEvent(JSON.parse(event.data));
              if (decoded.match_id !== matchId) throw Error();
              if (!first) {
                if (decoded.payload.type === "error") {
                  fail(decoded.payload.code);
                  return;
                }
                if (decoded.payload.type !== "snapshot") throw Error();
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
          detachProof = () =>
            context.signal.removeEventListener("abort", abort);
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
              if (
                closed ||
                !activated ||
                socket.readyState !== 1 ||
                input.match_id !== matchId
              )
                return false;
              const a = input.action,
                action =
                  a.type === "attack"
                    ? { type: a.type }
                    : { type: a.type, cell: a.cell };
              const wire = JSON.stringify({
                v: input.v,
                match_id: matchId,
                command_id: input.command_id,
                client_seq: input.client_seq,
                session_epoch: input.session_epoch,
                known_revision: input.known_revision,
                action,
              });
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
  };
}
