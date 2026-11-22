// @vitest-environment node
import { expect, test, vi } from "vitest";
import { authTestPorts, TEST_ACCOUNT } from "../../../test/auth-ports";
import { createRoomsHttp } from "./http";

const roomId = "b0000000-0000-4000-8000-000000000001";
const card = {
  room_id: roomId,
  title: "Evening race",
  capacity: 4,
  members: 1,
  phase: "waiting",
  accepting: true,
};

function reply(payload: unknown, status = 200) {
  return new Response(JSON.stringify({ v: 2, server_time_ms: 1, payload }), {
    status,
    headers: { "content-type": "application/json" },
  });
}

test("public rooms omit credentials and reject private fields in a list", async () => {
  const { auth } = authTestPorts();
  const fetcher = vi.fn<typeof fetch>();
  const page = { rooms: [card], next: null };
  fetcher.mockResolvedValueOnce(reply({ type: "list", page }));
  const api = createRoomsHttp(auth, fetcher);
  const signal = new AbortController().signal;
  expect(await api.list(null, signal)).toEqual(page);
  expect(fetcher).toHaveBeenCalledWith(
    "/api/v2/rooms",
    expect.objectContaining({
      credentials: "omit",
      cache: "no-store",
      method: "POST",
    }),
  );
  fetcher.mockResolvedValueOnce(
    reply({
      type: "list",
      page: { ...page, rooms: [{ ...card, code: "ABCD2345" }] },
    }),
  );
  await expect(api.list(null, signal)).rejects.toMatchObject({
    code: "malformed",
  });
});

test("an authenticated room command carries proof and preserves explicit server errors", async () => {
  const { auth } = authTestPorts(TEST_ACCOUNT, true);
  await auth.refresh();
  const account = auth.read().account!;
  const fetcher = vi.fn<typeof fetch>();
  fetcher.mockResolvedValueOnce(reply({ type: "current", room: null }));
  fetcher.mockResolvedValueOnce(reply({ type: "error", error: "full" }, 409));
  const api = createRoomsHttp(auth, fetcher);
  const owner = { accountId: account.id, revision: auth.revision() };
  const signal = new AbortController().signal;
  expect(await api.command(owner, { type: "status" }, signal)).toBeNull();
  expect(fetcher.mock.calls[0][1]).toMatchObject({
    credentials: "same-origin",
    headers: { "x-liar-csrf": "fixture-memory-csrf" },
  });
  await expect(
    api.command(
      owner,
      {
        type: "join",
        request_id: roomId,
        entrance: { kind: "public", room_id: roomId },
      },
      signal,
    ),
  ).rejects.toMatchObject({ code: "full" });
});
