import type {
  AuthAccount,
  AuthBootstrap,
  AuthErasure,
  AuthExport,
  AuthIdentity,
  AuthProviderStatus,
  AuthStart,
} from "@liar/protocol";
import { AuthError, PROVIDERS, isProvider, type AuthProvider } from "./types";
const invalid = (): never => {
  throw new AuthError("auth_invalid");
};
export function object(
  value: unknown,
  keys: readonly string[],
): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value))
    return invalid();
  const record = value as Record<string, unknown>;
  if (Object.keys(record).sort().join("|") !== [...keys].sort().join("|"))
    return invalid();
  return record;
}
function uuid(value: unknown): string {
  if (
    typeof value !== "string" ||
    !/^[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$/.test(value) ||
    value === "00000000-0000-0000-0000-000000000000"
  )
    return invalid();
  return value;
}
function time(value: unknown): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0)
    return invalid();
  return value;
}
export function decodeAccount(value: unknown): AuthAccount {
  const v = object(value, ["id", "nickname"]);
  if (
    v.nickname !== null &&
    (typeof v.nickname !== "string" ||
      v.nickname.length > 1024 ||
      /[\u0000-\u001f\u007f]/.test(v.nickname))
  )
    return invalid();
  return { id: uuid(v.id), nickname: v.nickname as string | null };
}
export function decodeIdentities(value: unknown): AuthIdentity[] {
  if (!Array.isArray(value) || value.length > PROVIDERS.length)
    return invalid();
  const identities = value.map((raw) => {
    const v = object(raw, ["provider", "linked_at"]);
    if (!isProvider(v.provider)) return invalid();
    return { provider: v.provider, linked_at: time(v.linked_at) };
  });
  if (new Set(identities.map((i) => i.provider)).size !== identities.length)
    return invalid();
  return identities;
}
export function decodeBootstrap(value: unknown): AuthBootstrap {
  const v = object(value, ["providers", "account", "session_revision", "csrf"]);
  if (!Array.isArray(v.providers) || v.providers.length !== PROVIDERS.length)
    return invalid();
  const providers: AuthProviderStatus[] = v.providers.map((raw) => {
    const p = object(raw, ["provider", "available"]);
    if (!isProvider(p.provider) || typeof p.available !== "boolean")
      return invalid();
    return { provider: p.provider, available: p.available };
  });
  if (new Set(providers.map((p) => p.provider)).size !== PROVIDERS.length)
    return invalid();
  const account = v.account === null ? null : decodeAccount(v.account);
  const session_revision =
    v.session_revision === null ? null : uuid(v.session_revision);
  if (Boolean(account) !== Boolean(session_revision)) return invalid();
  if (
    v.csrf !== null &&
    (typeof v.csrf !== "string" ||
      v.csrf.length === 0 ||
      v.csrf.length > 512 ||
      !/^[\x21-\x7e]+$/.test(v.csrf))
  )
    return invalid();
  if (account && v.csrf === null) return invalid();
  return {
    account,
    session_revision,
    providers,
    csrf: v.csrf as string | null,
  };
}
export function decodeStart(value: unknown, provider: AuthProvider): AuthStart {
  const v = object(value, ["authorize_url"]),
    config = PROVIDERS.find((p) => p.id === provider);
  if (
    !config ||
    typeof v.authorize_url !== "string" ||
    v.authorize_url.length > 8192
  )
    return invalid();
  let url: URL;
  try {
    url = new URL(v.authorize_url);
  } catch {
    return invalid();
  }
  if (
    url.origin !== config.origin ||
    url.pathname !== config.path ||
    url.username ||
    url.password ||
    url.hash
  )
    return invalid();
  return { authorize_url: url.href };
}
export function decodeExport(value: unknown): AuthExport {
  const v = object(value, [
    "account",
    "created_at",
    "last_seen_at",
    "identities",
  ]);
  const created_at = time(v.created_at),
    last_seen_at = time(v.last_seen_at);
  if (last_seen_at < created_at) return invalid();
  return {
    account: decodeAccount(v.account),
    created_at,
    last_seen_at,
    identities: decodeIdentities(v.identities),
  };
}
export function decodeErasure(value: unknown): AuthErasure {
  const v = object(value, ["manual_apple_disconnect"]);
  if (typeof v.manual_apple_disconnect !== "boolean") return invalid();
  return { manual_apple_disconnect: v.manual_apple_disconnect };
}
