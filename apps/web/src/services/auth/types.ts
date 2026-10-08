import type {
  AuthAccount,
  AuthBootstrap,
  AuthErasure,
  AuthExport,
  AuthIdentity,
  AuthProviderStatus,
  AuthStart,
} from "@liar/protocol";
import type { Locale } from "../locale";
import type { SubmissionSession } from "../pending-submissions";
export const PROVIDERS = [
  {
    id: "google",
    name: "Google",
    origin: "https://accounts.google.com",
    path: "/o/oauth2/v2/auth",
  },
  {
    id: "apple",
    name: "Apple",
    origin: "https://appleid.apple.com",
    path: "/auth/authorize",
  },
  {
    id: "kakao",
    name: "Kakao",
    origin: "https://kauth.kakao.com",
    path: "/oauth/authorize",
  },
  {
    id: "naver",
    name: "Naver",
    origin: "https://nid.naver.com",
    path: "/oauth2.0/authorize",
  },
] as const;
export type AuthProvider = (typeof PROVIDERS)[number]["id"];
export type AuthIntent = "login" | "link" | "reauth";
export type AuthReturn = "home" | "daily" | "friends" | "settings";
export type AuthCode =
  | "auth_required"
  | "auth_invalid"
  | "auth_unavailable"
  | "auth_conflict"
  | "reauth_required"
  | "auth_rate_limited";
export class AuthError extends Error {
  constructor(public readonly code: AuthCode) {
    super(code);
  }
}
export type AuthResult<T> =
  { ok: true; value: T } | { ok: false; code: AuthCode };
export interface AuthState {
  status: "loading" | "ready" | "unavailable";
  account: AuthAccount | null;
  providers: AuthProviderStatus[];
  identities: AuthIdentity[];
  working: boolean;
  error: AuthCode | null;
  notice: "logged_out" | "unlinked" | "deleted" | null;
  manualAppleDisconnect: boolean;
}
export interface AuthTransport {
  bootstrap(signal?: AbortSignal): Promise<AuthBootstrap>;
  identities(signal?: AbortSignal): Promise<AuthIdentity[]>;
  start(
    provider: AuthProvider,
    intent: AuthIntent,
    request: { locale: Locale; return_path: AuthReturn },
    csrf: string,
    signal?: AbortSignal,
  ): Promise<AuthStart>;
  nickname(
    value: string,
    csrf: string,
    signal?: AbortSignal,
  ): Promise<AuthAccount>;
  export(csrf: string, signal?: AbortSignal): Promise<AuthExport>;
  erase(csrf: string, signal?: AbortSignal): Promise<AuthErasure>;
  unlink(
    provider: AuthProvider,
    csrf: string,
    signal?: AbortSignal,
  ): Promise<AuthErasure>;
  logout(csrf: string, signal?: AbortSignal): Promise<void>;
}
export interface AuthPort extends SubmissionSession {
  read(): AuthState;
  subscribe(listener: () => void): () => void;
  refresh(): Promise<void>;
  invalidate(): void;
  start(
    provider: AuthProvider,
    intent: AuthIntent,
    locale: Locale,
    destination: AuthReturn,
  ): Promise<AuthResult<string>>;
  nickname(
    value: string,
    expectedAccount: string,
  ): Promise<AuthResult<AuthAccount>>;
  export(expectedAccount: string): Promise<AuthResult<AuthExport>>;
  erase(expectedAccount: string): Promise<AuthResult<AuthErasure>>;
  unlink(
    provider: AuthProvider,
    expectedAccount: string,
  ): Promise<AuthResult<AuthErasure>>;
  logout(): Promise<AuthResult<void>>;
  dispose(): void;
}
export function isProvider(value: unknown): value is AuthProvider {
  return PROVIDERS.some((p) => p.id === value);
}
export function authReturn(value: string | null): AuthReturn {
  return value === "daily" || value === "friends" || value === "settings"
    ? value
    : "home";
}
