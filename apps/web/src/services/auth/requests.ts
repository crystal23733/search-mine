import type { AuthBootstrap } from "@liar/protocol";
import { AuthError, type AuthCode } from "./types";

export type RequestCode =
  "unavailable" | "stale" | "cancelled" | "capacity" | "timeout";
export class AuthenticatedRequestError extends Error {
  constructor(public readonly code: RequestCode) {
    super(code);
  }
}
export interface RequestOwner {
  accountId: string;
  revision: number;
}
export interface RequestContext {
  csrf: string;
  signal: AbortSignal;
}
export interface AuthenticatedRequests {
  execute<T>(
    expected: RequestOwner,
    work: (context: RequestContext) => Promise<T>,
    signal?: AbortSignal,
  ): Promise<T>;
}
interface Authority extends RequestOwner {
  sessionId: string;
  generation: number;
}
interface Dependencies {
  authority(): Authority | null;
  bootstrap(signal: AbortSignal): Promise<AuthBootstrap>;
  invalidated(code: AuthCode): void;
}
class ProofFailure extends AuthError {}
export function createAuthenticatedRequests(
  dependencies: Dependencies,
): AuthenticatedRequests & { abortAll(): void } {
  const active = new Set<AbortController>();
  function matches(owner: Authority): boolean {
    const now = dependencies.authority();
    return (
      now !== null &&
      now.accountId === owner.accountId &&
      now.sessionId === owner.sessionId &&
      now.revision === owner.revision &&
      now.generation === owner.generation
    );
  }
  return {
    async execute(expected, work, signal) {
      if (signal?.aborted) throw new AuthenticatedRequestError("cancelled");
      const authority = dependencies.authority();
      if (!authority) throw new AuthenticatedRequestError("unavailable");
      const owner = { ...authority };
      if (
        expected.accountId !== owner.accountId ||
        expected.revision !== owner.revision
      )
        throw new AuthenticatedRequestError("stale");
      if (active.size >= 8) throw new AuthenticatedRequestError("capacity");
      const controller = new AbortController();
      active.add(controller);
      const cancel = () =>
        controller.abort(new AuthenticatedRequestError("cancelled"));
      signal?.addEventListener("abort", cancel, { once: true });
      let interrupted!: () => void;
      const interruption = new Promise<never>((_, reject) => {
        interrupted = () => reject(controller.signal.reason);
        controller.signal.addEventListener("abort", interrupted, {
          once: true,
        });
      });
      const timer = setTimeout(
        () => controller.abort(new AuthenticatedRequestError("timeout")),
        10_000,
      );
      function assertCurrent() {
        if (controller.signal.aborted) throw controller.signal.reason;
        if (!matches(owner)) throw new AuthenticatedRequestError("stale");
      }
      async function proof(): Promise<string> {
        assertCurrent();
        let fresh: AuthBootstrap;
        try {
          fresh = await dependencies.bootstrap(controller.signal);
        } catch (error) {
          assertCurrent();
          throw new ProofFailure(
            error instanceof AuthError ? error.code : "auth_unavailable",
          );
        }
        assertCurrent();
        if (
          fresh.account?.id !== owner.accountId ||
          !fresh.account.nickname ||
          fresh.session_revision !== owner.sessionId ||
          !fresh.csrf
        )
          throw new ProofFailure("auth_invalid");
        return fresh.csrf;
      }
      const run = async () => {
        const csrf = await proof();
        assertCurrent();
        const value = await work({ csrf, signal: controller.signal });
        assertCurrent();
        await proof();
        assertCurrent();
        return value;
      };
      try {
        return await Promise.race([run(), interruption]);
      } catch (error) {
        if (
          !controller.signal.aborted &&
          matches(owner) &&
          (error instanceof ProofFailure ||
            (error instanceof AuthError && error.code === "auth_required"))
        )
          dependencies.invalidated(error.code);
        throw error;
      } finally {
        clearTimeout(timer);
        signal?.removeEventListener("abort", cancel);
        controller.signal.removeEventListener("abort", interrupted);
        active.delete(controller);
      }
    },
    abortAll() {
      for (const controller of active)
        controller.abort(new AuthenticatedRequestError("stale"));
    },
  };
}
