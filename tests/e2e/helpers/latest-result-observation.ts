import { expect, type Page, type Route } from "@playwright/test";
import type { LatestResultResponse } from "../../../packages/protocol/src/index";

type Observation = {
  delivered: (body: LatestResultResponse) => void;
  failed: (error: unknown) => void;
};
type Observer = {
  ready: Promise<unknown>;
  pending: Observation | null;
};
const observers = new WeakMap<Page, Observer>();

function observerFor(page: Page): Observer {
  const existing = observers.get(page);
  if (existing) return existing;
  const observer: Observer = { ready: Promise.resolve(), pending: null };
  observers.set(page, observer);
  observer.ready = page.route(
    "**/api/v1/results/latest",
    async (route: Route) => {
      const pending = observer.pending;
      if (!pending) {
        await route.continue();
        return;
      }
      try {
        expect(route.request().method()).toBe("POST");
        expect(route.request().postDataJSON()).toEqual({ v: 1 });
        const response = await route.fetch({
          maxRedirects: 0,
          maxRetries: 0,
          timeout: 10000,
        });
        expect(response.status()).toBe(200);
        expect(response.headers()["cache-control"]).toBe("no-store");
        const body: LatestResultResponse = await response.json();
        // Keep interception stable through the app's post-response authority proof.
        await route.fulfill({ response });
        pending.delivered(body);
      } catch (error) {
        pending.failed(error);
        await route.abort();
      }
    },
  );
  return observer;
}

export async function observeLatestResult(
  page: Page,
  trigger: () => Promise<unknown>,
): Promise<LatestResultResponse> {
  const observer = observerFor(page);
  await observer.ready;
  if (observer.pending)
    throw new Error("Latest result observations must be serial");
  let delivered!: (body: LatestResultResponse) => void;
  let failed!: (error: unknown) => void;
  const observed = new Promise<LatestResultResponse>((resolve, reject) => {
    delivered = resolve;
    failed = reject;
  });
  const pending = { delivered, failed };
  observer.pending = pending;
  try {
    const [, body] = await Promise.all([trigger(), observed]);
    return body;
  } finally {
    if (observer.pending === pending) observer.pending = null;
  }
}
