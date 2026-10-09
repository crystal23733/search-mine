import { expect, test, vi } from "vitest";
import type { AuthenticatedRequests, RequestOwner } from "../auth/requests";
import { AuthError } from "../auth/types";
import { createLobbyHttp } from "./http";
const owner: RequestOwner = {
  accountId: "11111111-1111-4111-8111-111111111111",
  revision: 1,
};
const auth: AuthenticatedRequests = {
  execute: async (_owner, work, signal) =>
    work({ csrf: "fresh-proof", signal: signal! }),
};
test("uses fixed same-origin POST with private fresh proof and allowlisted Rust command only", async () => {
  const fetcher = vi.fn(
    async (_input: RequestInfo | URL, _init?: RequestInit) =>
      new Response(
        JSON.stringify({ v: 1, server_time_ms: 0, state: { type: "idle" } }),
        { headers: { "content-type": "application/json" } },
      ),
  );
  const signal = new AbortController().signal;
  expect(
    await createLobbyHttp(auth, fetcher)(owner, { type: "status" }, signal),
  ).toEqual({ v: 1, server_time_ms: 0, state: { type: "idle" } });
  expect(fetcher.mock.calls[0]).toEqual([
    "/api/v1/lobby",
    expect.objectContaining({
      method: "POST",
      credentials: "same-origin",
      redirect: "error",
      cache: "no-store",
      signal,
      headers: {
        "content-type": "application/json",
        "x-liar-csrf": "fresh-proof",
      },
      body: '{"v":1,"command":{"type":"status"}}',
    }),
  ]);
});
test("keeps domain rejection, rejects secret/oversize/MIME replies, maps actual unauthorized to auth invalidation", async () => {
  const run = (raw: string, status = 200, mime = "application/json") =>
    createLobbyHttp(
      auth,
      vi.fn(
        async () =>
          new Response(raw, { status, headers: { "content-type": mime } }),
      ),
    )(owner, { type: "status" }, new AbortController().signal);
  await expect(run('{"error":"unauthorized"}', 401)).rejects.toBeInstanceOf(
    AuthError,
  );
  await expect(run('{"error":"busy"}', 409)).rejects.toMatchObject({
    code: "busy",
  });
  for (const [raw, mime] of [
    [
      JSON.stringify({
        v: 1,
        server_time_ms: 0,
        state: { type: "idle" },
        seed: "private",
      }),
      "application/json",
    ],
    [" ".repeat(131073), "application/json"],
    ["{}", "text/html"],
  ])
    await expect(run(raw, 200, mime)).rejects.toThrow();
});
