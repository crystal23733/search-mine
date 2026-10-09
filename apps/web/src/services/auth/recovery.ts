import type { AuthBootstrap, AuthIdentity } from "@liar/protocol";
import type { RequestOwner } from "./requests";
import {
  AuthConnectionError,
  AuthError,
  type AuthResult,
  type AuthCode,
  type AuthTransport,
} from "./types";

interface Identity extends RequestOwner {
  sessionId: string;
  authorityKey: string;
}
interface Candidate extends Identity {
  until: number;
}

export class SessionRecovery {
  private candidate: Candidate | null = null;
  private expiry: ReturnType<typeof setTimeout> | undefined;
  private attempt: AbortController | null = null;
  constructor(
    private transport: AuthTransport,
    private online: () => boolean,
    private now: () => number,
    private restored: (
      proof: AuthBootstrap,
      identities: AuthIdentity[],
    ) => void,
    private discarded: (code?: AuthCode) => void,
  ) {}
  key(): string | null {
    return this.candidate?.authorityKey ?? null;
  }
  owner(): RequestOwner | null {
    if (this.candidate && this.now() >= this.candidate.until) this.discarded();
    return this.candidate
      ? {
          accountId: this.candidate.accountId,
          revision: this.candidate.revision,
        }
      : null;
  }
  capture(identity: Identity) {
    if (this.candidate) return;
    this.candidate = { ...identity, until: this.now() + 30_000 };
    this.expiry = setTimeout(() => this.discarded(), 30_000);
  }
  interrupt() {
    this.attempt?.abort(new AuthConnectionError());
  }
  clear() {
    clearTimeout(this.expiry);
    this.candidate = null;
    this.attempt?.abort(new AuthError("auth_invalid"));
    this.attempt = null;
  }
  async resume(expected: RequestOwner): Promise<AuthResult<void>> {
    const owner = this.owner(),
      candidate = this.candidate;
    if (
      !owner ||
      !candidate ||
      owner.accountId !== expected.accountId ||
      owner.revision !== expected.revision
    )
      return { ok: false, code: "auth_invalid" };
    if (!this.online() || this.attempt)
      return { ok: false, code: "auth_unavailable" };
    const controller = new AbortController();
    this.attempt = controller;
    const assertCurrent = () => {
      if (controller.signal.aborted) throw controller.signal.reason;
      if (this.candidate !== candidate || this.attempt !== controller)
        throw new AuthError("auth_invalid");
      if (this.now() >= candidate.until) {
        this.discarded();
        throw new AuthError("auth_invalid");
      }
      if (!this.online()) throw new AuthConnectionError();
    };
    const check = (proof: AuthBootstrap) => {
      assertCurrent();
      if (
        proof.account?.id !== candidate.accountId ||
        !proof.account.nickname ||
        proof.session_revision !== candidate.sessionId ||
        !proof.csrf
      )
        throw new AuthError("auth_invalid");
    };
    let interrupted!: () => void;
    const interruption = new Promise<never>((_, reject) => {
      interrupted = () => reject(controller.signal.reason);
      controller.signal.addEventListener("abort", interrupted, { once: true });
    });
    const timer = setTimeout(
      () => controller.abort(new AuthConnectionError()),
      Math.min(10_000, candidate.until - this.now()),
    );
    const prove = async () => {
      const first = await this.transport.bootstrap(controller.signal);
      check(first);
      const identities = await this.transport.identities(controller.signal);
      assertCurrent();
      const accepted = await this.transport.bootstrap(controller.signal);
      check(accepted);
      return { accepted, identities };
    };
    try {
      const { accepted, identities } = await Promise.race([
        prove(),
        interruption,
      ]);
      assertCurrent();
      controller.signal.removeEventListener("abort", interrupted);
      this.clear();
      this.restored(accepted, identities);
      return { ok: true, value: undefined };
    } catch (error) {
      if (
        this.candidate === candidate &&
        !(error instanceof AuthConnectionError)
      )
        this.discarded(
          error instanceof AuthError ? error.code : "auth_unavailable",
        );
      return {
        ok: false,
        code: error instanceof AuthError ? error.code : "auth_unavailable",
      };
    } finally {
      clearTimeout(timer);
      controller.signal.removeEventListener("abort", interrupted);
      if (this.attempt === controller) this.attempt = null;
    }
  }
}
