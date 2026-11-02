import type { BrowserContext, Page, Response } from "@playwright/test";
import type { MeHistory } from "../../../packages/protocol/src/index";

interface Observation {
  body: string;
  result: Promise<
    | { ok: true; status: number; cache: string | null; value: unknown }
    | { ok: false; error: string }
  >;
}
declare global {
  interface Window {
    __meHistoryObservations?: Observation[];
  }
}

export async function observeMeHistory(context: BrowserContext) {
  await context.addInitScript(() => {
    const observations: Observation[] = [];
    Object.defineProperty(window, "__meHistoryObservations", {
      value: observations,
    });
    const original = window.fetch;
    window.fetch = function (...args) {
      const pending = Reflect.apply(original, this, args);
      const [input, init] = args;
      if (
        input === "/api/v1/me/history" &&
        init?.method === "POST" &&
        typeof init.body === "string"
      ) {
        const result: Observation["result"] = pending
          .then(async (response) => {
            const copy = response.clone();
            return {
              ok: true as const,
              status: copy.status,
              cache: copy.headers.get("cache-control"),
              value: await copy.json(),
            };
          })
          .catch((error: unknown) => ({ ok: false, error: String(error) }));
        observations.push({ body: init.body, result });
        if (observations.length > 32) observations.shift();
      }
      // Preserve the app's original promise, response and cancellation behavior.
      return pending;
    };
  });
}

export async function observedMeHistory(page: Page, response: Response) {
  const observed = await page.evaluate(async (body) => {
    const entries = window.__meHistoryObservations?.filter(
      (entry) => entry.body === body,
    );
    if (entries?.length !== 1)
      throw Error(
        `History observation count ${entries?.length ?? 0}; expected exactly one`,
      );
    return await entries[0].result;
  }, response.request().postData());
  if (!observed.ok) throw Error(observed.error);
  if (observed.status !== response.status() || observed.cache !== "no-store")
    throw Error("Unexpected history status or cache policy");
  return observed.value as MeHistory;
}
