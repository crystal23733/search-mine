import { expect, test, vi } from "vitest";
import { createAuthHttp } from "./http";
import { PROVIDERS, AuthError } from "./types";
const account = {
  id: "a0000000-0000-4000-8000-000000000001",
  nickname: "player",
};
const bootstrap = {
  providers: PROVIDERS.map((p) => ({ provider: p.id, available: true })),
  account,
  session_revision: "b0000000-0000-4000-8000-000000000001",
  csrf: "fixture-memory-csrf",
};
const json = (value: unknown, status = 200) =>
  new Response(JSON.stringify(value), {
    status,
    headers: { "content-type": "application/json" },
  });
test("same-origin no-store requests send only minimal data and memory CSRF", async () => {
  const fetcher = vi
    .fn<typeof fetch>()
    .mockResolvedValueOnce(json(bootstrap))
    .mockResolvedValueOnce(
      json({
        authorize_url:
          "https://accounts.google.com/o/oauth2/v2/auth?state=fixture",
      }),
    )
    .mockResolvedValueOnce(json({ ...account, nickname: "é探偵" }));
  const http = createAuthHttp(fetcher);
  expect(await http.bootstrap()).toEqual(bootstrap);
  expect(
    await http.start(
      "google",
      "login",
      { locale: "ko", return_path: "friends" },
      bootstrap.csrf,
    ),
  ).toEqual({
    authorize_url: "https://accounts.google.com/o/oauth2/v2/auth?state=fixture",
  });
  expect(await http.nickname("e\u0301探偵", bootstrap.csrf)).toEqual({
    ...account,
    nickname: "é探偵",
  });
  expect(fetcher.mock.calls.map((c) => c[0])).toEqual([
    "/api/v1/auth/bootstrap",
    "/api/v1/auth/google/start",
    "/api/v1/me/nickname",
  ]);
  for (const [, init] of fetcher.mock.calls) {
    expect(init).toMatchObject({
      credentials: "same-origin",
      cache: "no-store",
      redirect: "error",
    });
    expect(init?.signal).toBeInstanceOf(AbortSignal);
  }
  expect(JSON.parse(String(fetcher.mock.calls[1][1]?.body))).toEqual({
    locale: "ko",
    return_path: "friends",
  });
  expect(
    new Headers(fetcher.mock.calls[1][1]?.headers).get("x-liar-csrf"),
  ).toBe(bootstrap.csrf);
});
test("unknown profile data, mismatched session authority and unsafe redirects never reach state", async () => {
  for (const body of [
    { ...bootstrap, email: "private@example.com" },
    { ...bootstrap, account: { ...account, subject: "raw" } },
    { ...bootstrap, session_revision: null },
  ]) {
    const http = createAuthHttp(
      vi.fn<typeof fetch>().mockResolvedValue(json(body)),
    );
    await expect(http.bootstrap()).rejects.toEqual(
      new AuthError("auth_invalid"),
    );
  }
  for (const authorize_url of [
    "https://evil.example/",
    "javascript:alert(1)",
    "https://accounts.google.com@evil.example/o/oauth2/v2/auth",
    "https://accounts.google.com/o/oauth2/v2/auth#token",
  ]) {
    const http = createAuthHttp(
      vi.fn<typeof fetch>().mockResolvedValue(json({ authorize_url })),
    );
    await expect(
      http.start(
        "google",
        "login",
        { locale: "en", return_path: "home" },
        "csrf",
      ),
    ).rejects.toEqual(new AuthError("auth_invalid"));
  }
});
test("stable errors and stalled fetch have bounded completion without echoing provider bodies", async () => {
  const unauthorized = createAuthHttp(
    vi
      .fn<typeof fetch>()
      .mockResolvedValue(json({ code: "auth_required" }, 401)),
  );
  await expect(unauthorized.export("csrf")).rejects.toEqual(
    new AuthError("auth_required"),
  );
  vi.useFakeTimers();
  try {
    const stalled = createAuthHttp(
      vi.fn<typeof fetch>(() => new Promise(() => {})),
    );
    const failure = expect(stalled.bootstrap()).rejects.toEqual(
      new AuthError("auth_unavailable"),
    );
    await vi.advanceTimersByTimeAsync(10000);
    await failure;
  } finally {
    vi.useRealTimers();
  }
});
