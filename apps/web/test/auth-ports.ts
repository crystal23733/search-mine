import { vi } from "vitest";
import type { AuthAccount } from "@liar/protocol";
import { createAuth } from "../src/services/auth/session";
import {
  PROVIDERS,
  AuthError,
  type AuthTransport,
} from "../src/services/auth/types";
export const TEST_ACCOUNT = {
  id: "a0000000-0000-4000-8000-000000000001",
  nickname: "Player",
};
export function authTestPorts(
  initial: AuthAccount | null = null,
  enabled = false,
) {
  let account = initial;
  let identities = initial ? [{ provider: "google", linked_at: 1 }] : [];
  const snapshot = () => ({
    account,
    providers: PROVIDERS.map((p) => ({ provider: p.id, available: enabled })),
    session_revision: account ? "b0000000-0000-4000-8000-000000000001" : null,
    csrf: enabled ? "fixture-memory-csrf" : null,
  });
  const authTransport: AuthTransport = {
    bootstrap: vi.fn(async () => snapshot()),
    identities: vi.fn(async () => identities),
    start: vi.fn(async (provider) => {
      const p = PROVIDERS.find((p) => p.id === provider)!;
      return { authorize_url: `${p.origin}${p.path}?state=fixture` };
    }),
    nickname: vi.fn(async (value) => {
      if (!account) throw new AuthError("auth_required");
      account = { ...account, nickname: value.normalize("NFC") };
      return account;
    }),
    export: vi.fn(async () => {
      if (!account) throw new AuthError("auth_required");
      return { account, identities, created_at: 1, last_seen_at: 2 };
    }),
    erase: vi.fn(async () => {
      account = null;
      identities = [];
      return { manual_apple_disconnect: false };
    }),
    unlink: vi.fn(async () => {
      account = null;
      identities = [];
      return { manual_apple_disconnect: false };
    }),
    logout: vi.fn(async () => {
      account = null;
    }),
  };
  return {
    auth: createAuth(authTransport),
    authTransport,
    authEffects: { redirect: vi.fn(), download: vi.fn(async () => {}) },
  };
}
