import type { AuthAccount, AuthErasure, AuthExport } from "@liar/protocol";
import { createAuthenticatedRequests } from "./requests";
import { SessionRecovery } from "./recovery";
import {
  AuthError,
  PROVIDERS,
  type AuthCode,
  type AuthPort,
  type AuthResult,
  type AuthState,
  type AuthTransport,
} from "./types";
class OwnershipChanged extends AuthError {
  constructor() {
    super("auth_invalid");
  }
}
type Completion<T> = { result: AuthResult<T>; generation: number };
export function createAuth(
  transport: AuthTransport,
  online: () => boolean = () => true,
  mutated: () => void = () => {},
  now: () => number = () => performance.now(),
): AuthPort {
  let state: AuthState = {
    status: "loading",
    account: null,
    identities: [],
    providers: PROVIDERS.map((p) => ({ provider: p.id, available: false })),
    working: false,
    error: null,
    notice: null,
    manualAppleDisconnect: false,
  };
  let csrf: string | null = null,
    sessionId: string | null = null,
    generation = 0,
    revision = 0,
    authorityKey = "",
    disposed = false;
  let active: AbortController | undefined;
  const listeners = new Set<() => void>();
  const connected = () =>
    !disposed &&
    online() &&
    state.status === "ready" &&
    !state.working &&
    state.account !== null;
  const recovery = new SessionRecovery(
    transport,
    online,
    now,
    (accepted, identities) => {
      csrf = accepted.csrf;
      sessionId = accepted.session_revision;
      publish({
        status: "ready",
        account: accepted.account,
        providers: accepted.providers,
        identities,
        working: false,
        error: null,
      });
    },
    (failure = "auth_unavailable") => {
      begin();
      clear("unavailable", failure);
    },
  );
  function publish(patch: Partial<AuthState>) {
    state = { ...state, ...patch };
    const key =
      recovery.key() ??
      `${connected()}/${state.account?.id ?? ""}/${sessionId ?? ""}`;
    if (key !== authorityKey) {
      authorityKey = key;
      revision++;
    }
    for (const listener of listeners) listener();
  }
  function clear(status: AuthState["status"], error: AuthCode | null = null) {
    csrf = null;
    sessionId = null;
    publish({
      status,
      account: null,
      identities: [],
      working: false,
      error,
      notice: null,
      manualAppleDisconnect: false,
    });
  }
  function begin(preserveRecovery = false) {
    if (!preserveRecovery) recovery.clear();
    requests.abortAll();
    active?.abort();
    active = new AbortController();
    generation++;
    return { generation, signal: active.signal };
  }
  const current = (request: { generation: number }) =>
    !disposed && request.generation === generation;
  const code = (error: unknown): AuthCode =>
    error instanceof AuthError ? error.code : "auth_unavailable";
  const notification = () => {
    try {
      mutated();
    } catch {
      /* A peer notification cannot undo a committed server effect. */
    }
  };
  async function refresh(): Promise<void> {
    if (disposed) return;
    const request = begin();
    clear("loading");
    if (!online()) {
      clear("unavailable", "auth_unavailable");
      return;
    }
    try {
      const first = await transport.bootstrap(request.signal);
      if (!current(request)) return;
      let accepted = first,
        identities: AuthState["identities"] = [];
      if (first.account) {
        identities = await transport.identities(request.signal);
        if (!current(request)) return;
        accepted = await transport.bootstrap(request.signal);
        if (!current(request)) return;
        if (
          accepted.account?.id !== first.account.id ||
          accepted.session_revision !== first.session_revision
        )
          throw new AuthError("auth_required");
      }
      csrf = accepted.csrf;
      sessionId = accepted.session_revision;
      publish({
        status: "ready",
        account: accepted.account,
        providers: accepted.providers,
        identities,
        working: false,
        error: null,
      });
    } catch (error) {
      if (current(request)) clear("unavailable", code(error));
    }
  }
  async function operate<T>(
    expected: string | undefined,
    work: (csrf: string, signal: AbortSignal) => Promise<T>,
  ): Promise<Completion<T>> {
    if (recovery.owner()) {
      begin();
      clear("unavailable", "auth_unavailable");
    }
    const complete = (
      result: AuthResult<T>,
      at = generation,
    ): Completion<T> => ({ result, generation: at });
    if (expected !== undefined && state.account?.id !== expected) {
      publish({ error: "auth_invalid" });
      return complete({ ok: false, code: "auth_invalid" });
    }
    if (
      disposed ||
      !online() ||
      state.status !== "ready" ||
      state.working ||
      !csrf
    )
      return complete({ ok: false, code: "auth_unavailable" });
    const owner = state.account?.id ?? null,
      session = sessionId,
      request = begin();
    publish({
      working: true,
      error: null,
      notice: null,
      manualAppleDisconnect: false,
    });
    try {
      const fresh = await transport.bootstrap(request.signal);
      if (!current(request))
        return complete({ ok: false, code: "auth_invalid" });
      if (
        (fresh.account?.id ?? null) !== owner ||
        fresh.session_revision !== session ||
        !fresh.csrf
      )
        throw new OwnershipChanged();
      csrf = fresh.csrf;
      const value = await work(csrf, request.signal);
      if (!current(request))
        return complete({ ok: false, code: "auth_invalid" });
      publish({ working: false });
      return complete({ ok: true, value }, request.generation);
    } catch (error) {
      const failure = code(error);
      if (current(request)) {
        if (
          failure === "auth_required" ||
          failure === "auth_unavailable" ||
          error instanceof OwnershipChanged
        )
          clear("unavailable", failure);
        else publish({ working: false, error: failure });
      }
      return complete({ ok: false, code: failure });
    }
  }
  function requireOwner(account: AuthAccount, expected: string) {
    if (account.id !== expected) {
      throw new OwnershipChanged();
    }
  }
  function checked<T>(completion: Completion<T>): AuthResult<T> {
    return completion.result.ok && !current(completion)
      ? { ok: false, code: "auth_invalid" }
      : completion.result;
  }
  async function erased(
    completion: Completion<AuthErasure>,
    notice: "deleted" | "unlinked",
  ): Promise<AuthResult<AuthErasure>> {
    const result = checked(completion);
    if (!result.ok) return result;
    clear("ready");
    notification();
    if (current(completion)) {
      const refreshed = refresh(),
        at = generation;
      await refreshed;
      if (current({ generation: at }))
        publish({
          notice,
          manualAppleDisconnect: result.value.manual_apple_disconnect,
        });
    }
    return result;
  }
  const requests = createAuthenticatedRequests({
    authority: () =>
      connected() && state.account?.nickname && sessionId
        ? {
            accountId: state.account.id,
            revision,
            generation,
            sessionId,
          }
        : null,
    bootstrap: (signal) => transport.bootstrap(signal),
    invalidated(failure, retryable) {
      if (retryable) {
        suspend();
        return;
      }
      begin();
      clear("unavailable", failure);
    },
  });
  function suspend() {
    if (disposed) return;
    if (
      !recovery.owner() &&
      state.status === "ready" &&
      !state.working &&
      state.account?.nickname &&
      sessionId
    )
      recovery.capture({
        accountId: state.account.id,
        revision,
        sessionId,
        authorityKey,
      });
    recovery.interrupt();
    begin(true);
    clear("unavailable", "auth_unavailable");
  }
  return {
    execute: requests.execute,
    read: () => ({
      ...state,
      account: state.account ? { ...state.account } : null,
      providers: state.providers.map((p) => ({ ...p })),
      identities: state.identities.map((i) => ({ ...i })),
    }),
    account: () => state.account?.id ?? null,
    revision: () => revision,
    connected,
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    refresh,
    suspend,
    recoveryOwner: () => recovery.owner(),
    resume: (expected) => recovery.resume(expected),
    invalidate() {
      if (disposed) return;
      begin();
      clear("unavailable", "auth_unavailable");
    },
    async start(provider, intent, locale, destination, inviteCode) {
      if (recovery.owner()) {
        begin();
        clear("unavailable", "auth_unavailable");
      }
      if (!state.providers.some((p) => p.provider === provider && p.available))
        return { ok: false, code: "auth_unavailable" };
      const owner = intent === "login" ? undefined : state.account?.id;
      if (intent !== "login" && !owner)
        return { ok: false, code: "auth_required" };
      if (
        intent === "reauth" &&
        !state.identities.some((i) => i.provider === provider)
      )
        return { ok: false, code: "auth_invalid" };
      const result = checked(
        await operate(owner, (proof, signal) =>
          transport.start(
            provider,
            intent,
            {
              locale,
              return_path: destination,
              ...(inviteCode === undefined ? {} : { invite_code: inviteCode }),
            },
            proof,
            signal,
          ),
        ),
      );
      return result.ok
        ? { ok: true, value: result.value.authorize_url }
        : result;
    },
    async nickname(value, expected) {
      const result = checked(
        await operate(expected, async (proof, signal) => {
          const accepted = await transport.nickname(value, proof, signal);
          requireOwner(accepted, expected);
          return accepted;
        }),
      );
      if (result.ok) {
        publish({ account: result.value });
        notification();
      }
      return result;
    },
    export: async (expected) =>
      checked(
        await operate<AuthExport>(expected, async (proof, signal) => {
          const value = await transport.export(proof, signal);
          requireOwner(value.account, expected);
          return value;
        }),
      ),
    erase: async (expected) =>
      erased(
        await operate(expected, (proof, signal) =>
          transport.erase(proof, signal),
        ),
        "deleted",
      ),
    unlink: async (provider, expected) =>
      erased(
        await operate(expected, (proof, signal) =>
          transport.unlink(provider, proof, signal),
        ),
        "unlinked",
      ),
    async logout() {
      const completion = await operate(undefined, (proof, signal) =>
        transport.logout(proof, signal),
      );
      const result = checked(completion);
      if (result.ok) {
        clear("ready");
        notification();
        if (current(completion)) {
          const refreshed = refresh(),
            at = generation;
          await refreshed;
          if (current({ generation: at })) publish({ notice: "logged_out" });
        }
      }
      return result;
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      recovery.clear();
      generation++;
      active?.abort();
      requests.abortAll();
      csrf = null;
      sessionId = null;
      listeners.clear();
      state = { ...state, account: null, identities: [] };
      revision++;
    },
  };
}
