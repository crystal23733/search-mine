export interface UpdateClient {
  id: string;
  prepare(token: string): Promise<boolean>;
  release(token: string): void;
}
export interface UpdateBoundary {
  clients(): Promise<UpdateClient[]>;
  token(): string;
  now(): number;
  commit(): Promise<void>;
}
function prepare(client: UpdateClient, token: string): Promise<boolean> {
  return new Promise((resolve) => {
    const timer = setTimeout(() => resolve(false), 2000);
    void Promise.resolve()
      .then(() => client.prepare(token))
      .then((value) => {
        clearTimeout(timer);
        resolve(value === true);
      })
      .catch(() => {
        clearTimeout(timer);
        resolve(false);
      });
  });
}
export async function coordinateIdleClients(
  boundary: UpdateBoundary,
): Promise<boolean> {
  let clients: UpdateClient[] = [];
  const token = boundary.token();
  const started = boundary.now();
  let committed = false;
  try {
    clients = [...(await boundary.clients())];
    if (
      clients.length === 0 ||
      clients.length > 16 ||
      new Set(clients.map((client) => client.id)).size !== clients.length
    )
      return false;
    const ready = await Promise.all(
      clients.map((client) => prepare(client, token)),
    );
    if (ready.some((value) => !value)) return false;
    const current = await boundary.clients();
    const ids = new Set(clients.map((client) => client.id));
    const elapsed = boundary.now() - started;
    if (
      elapsed < 0 ||
      elapsed >= 5000 ||
      current.length !== ids.size ||
      new Set(current.map((client) => client.id)).size !== ids.size ||
      current.some((client) => !ids.has(client.id))
    )
      return false;
    await boundary.commit();
    committed = true;
    return true;
  } catch {
    return false;
  } finally {
    if (!committed)
      for (const client of clients) {
        try {
          client.release(token);
        } catch {
          /* Other tabs must still unlock. */
        }
      }
  }
}
