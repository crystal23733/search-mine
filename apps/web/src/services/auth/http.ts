import {
  AuthError,
  AuthConnectionError,
  isProvider,
  type AuthCode,
  type AuthTransport,
} from "./types";
import {
  decodeAccount,
  decodeBootstrap,
  decodeErasure,
  decodeExport,
  decodeIdentities,
  decodeStart,
  object,
} from "./decode";
export function createAuthHttp(fetcher: typeof fetch): AuthTransport {
  async function request<T>(
    path: string,
    decode: (value: unknown) => T,
    csrf?: string,
    body?: unknown,
    signal?: AbortSignal,
    method = "POST",
  ): Promise<T> {
    const controller = new AbortController();
    const abort = () => controller.abort();
    signal?.addEventListener("abort", abort, { once: true });
    if (signal?.aborted) controller.abort();
    let timer: ReturnType<typeof setTimeout> | undefined;
    const execute = async () => {
      const response = await fetcher(path, {
        method,
        credentials: "same-origin",
        cache: "no-store",
        redirect: "error",
        signal: controller.signal,
        headers: {
          ...(body === undefined ? {} : { "content-type": "application/json" }),
          ...(csrf ? { "x-liar-csrf": csrf } : {}),
        },
        ...(body === undefined ? {} : { body: JSON.stringify(body) }),
      }).catch(() => {
        throw new AuthConnectionError();
      });
      if (response.redirected) throw new AuthError("auth_unavailable");
      if (response.status === 204 && path === "/api/v1/auth/logout")
        return undefined as T;
      if (
        response.headers
          .get("content-type")
          ?.toLowerCase()
          .split(";")[0]
          .trim() !== "application/json"
      )
        throw new AuthError("auth_unavailable");
      if (!response.body) throw new AuthError("auth_unavailable");
      const reader = response.body.getReader();
      const chunks: Uint8Array[] = [];
      let size = 0;
      try {
        for (;;) {
          const part = await reader.read().catch(() => {
            throw new AuthConnectionError();
          });
          if (part.done) break;
          size += part.value.byteLength;
          if (size > 65536) {
            await reader.cancel();
            throw new AuthError("auth_unavailable");
          }
          chunks.push(part.value);
        }
      } finally {
        reader.releaseLock();
      }
      const bytes = new Uint8Array(size);
      let offset = 0;
      for (const chunk of chunks) {
        bytes.set(chunk, offset);
        offset += chunk.byteLength;
      }
      let raw: unknown;
      try {
        raw = JSON.parse(
          new TextDecoder("utf-8", { fatal: true }).decode(bytes),
        );
      } catch {
        throw new AuthError("auth_unavailable");
      }
      if (response.status !== 200) {
        let code: AuthCode = "auth_unavailable";
        let v: Record<string, unknown>;
        try {
          v = object(raw, ["code"]);
        } catch {
          throw new AuthError("auth_unavailable");
        }
        if (response.status === 401 && v.code === "auth_required")
          code = "auth_required";
        else if (response.status === 400 && v.code === "auth_invalid")
          code = "auth_invalid";
        else if (
          response.status === 409 &&
          (v.code === "auth_conflict" || v.code === "reauth_required")
        )
          code = v.code;
        else if (response.status === 429) code = "auth_rate_limited";
        throw new AuthError(code);
      }
      try {
        return decode(raw);
      } catch {
        throw new AuthError("auth_unavailable");
      }
    };
    try {
      return await Promise.race([
        execute(),
        new Promise<never>((_, reject) => {
          timer = setTimeout(() => {
            controller.abort();
            reject(new AuthConnectionError());
          }, 10000);
        }),
      ]);
    } catch (error) {
      throw error instanceof AuthError
        ? error
        : new AuthError("auth_unavailable");
    } finally {
      clearTimeout(timer);
      signal?.removeEventListener("abort", abort);
      controller.abort();
    }
  }
  const providerPath = (provider: unknown) => {
    if (!isProvider(provider)) throw new AuthError("auth_invalid");
    return provider;
  };
  return {
    bootstrap: (signal) =>
      request(
        "/api/v1/auth/bootstrap",
        decodeBootstrap,
        undefined,
        undefined,
        signal,
        "GET",
      ),
    identities: (signal) =>
      request(
        "/api/v1/me/identities",
        decodeIdentities,
        undefined,
        undefined,
        signal,
        "GET",
      ),
    start: (provider, intent, body, csrf, signal) => {
      const p = providerPath(provider);
      const path =
        intent === "link"
          ? `/api/v1/me/identities/${p}/link`
          : `/api/v1/auth/${p}/${intent === "reauth" ? "reauth" : "start"}`;
      return request(path, (v) => decodeStart(v, p), csrf, body, signal);
    },
    nickname: (nickname, csrf, signal) =>
      request("/api/v1/me", decodeAccount, csrf, { nickname }, signal, "PATCH"),
    export: (csrf, signal) =>
      request("/api/v1/me/export", decodeExport, csrf, {}, signal),
    erase: (csrf, signal) =>
      request("/api/v1/me", decodeErasure, csrf, {}, signal, "DELETE"),
    unlink: (provider, csrf, signal) =>
      request(
        `/api/v1/me/identities/${providerPath(provider)}`,
        decodeErasure,
        csrf,
        {},
        signal,
        "DELETE",
      ),
    logout: (csrf, signal) =>
      request("/api/v1/auth/logout", () => undefined, csrf, {}, signal),
  };
}
