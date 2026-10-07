import { expect, test, vi } from "vitest";
import {
  coordinateIdleClients,
  type UpdateBoundary,
  type UpdateClient,
} from "./update-coordinator";
const client = (id: string, idle = true): UpdateClient => ({
  id,
  prepare: vi.fn(async () => idle),
  release: vi.fn(),
});
function boundary(clients: UpdateClient[]): UpdateBoundary {
  return {
    clients: async () => clients,
    token: () => "token",
    now: () => performance.now(),
    commit: vi.fn(async () => {}),
  };
}
test("only all idle clients can commit an update after the client set is rechecked", async () => {
  const clients = [client("one"), client("two")];
  const ports = boundary(clients);
  expect(await coordinateIdleClients(ports)).toBe(true);
  expect(ports.commit).toHaveBeenCalledTimes(1);
  expect(clients[0].prepare).toHaveBeenCalledWith("token");
  expect(clients[0].release).not.toHaveBeenCalled();
  const busy = [client("one"), client("two", false)];
  const blocked = boundary(busy);
  expect(await coordinateIdleClients(blocked)).toBe(false);
  expect(blocked.commit).not.toHaveBeenCalled();
  expect(busy[0].release).toHaveBeenCalledWith("token");
  let calls = 0;
  const changed = boundary(clients);
  changed.clients = async () =>
    calls++ === 0 ? clients : [...clients, client("new")];
  expect(await coordinateIdleClients(changed)).toBe(false);
  expect(changed.commit).not.toHaveBeenCalled();
});
test("missing or late clients and a failed activation release prepared tabs without committing later", async () => {
  vi.useFakeTimers();
  try {
    const waiting = client("silent");
    waiting.prepare = () => new Promise(() => {});
    const ports = boundary([client("ready"), waiting]);
    const result = coordinateIdleClients(ports);
    await vi.advanceTimersByTimeAsync(2000);
    expect(await result).toBe(false);
    expect(ports.commit).not.toHaveBeenCalled();
  } finally {
    vi.useRealTimers();
  }
  const ports = boundary([client("ready")]);
  ports.commit = async () => {
    throw Error("failed");
  };
  expect(await coordinateIdleClients(ports)).toBe(false);
  const late = boundary([client("ready")]);
  let call = 0;
  late.now = () => (call++ === 0 ? 0 : 6000);
  expect(await coordinateIdleClients(late)).toBe(false);
  expect(late.commit).not.toHaveBeenCalled();
});
