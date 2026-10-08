import { createAuth } from "../src/services/auth/session";
import { AuthError, PROVIDERS } from "../src/services/auth/types";
export function guestAuthPorts() {
  const unavailable = async (): Promise<never> => {
    throw new AuthError("auth_unavailable");
  };
  return {
    auth: createAuth({
      bootstrap: async () => ({
        account: null,
        session_revision: null,
        csrf: null,
        providers: PROVIDERS.map((p) => ({ provider: p.id, available: false })),
      }),
      identities: async () => [],
      start: unavailable,
      nickname: unavailable,
      export: unavailable,
      erase: unavailable,
      unlink: unavailable,
      logout: unavailable,
    }),
    authEffects: {
      redirect: () => {
        throw new AuthError("auth_unavailable");
      },
      download: unavailable,
    },
  };
}
