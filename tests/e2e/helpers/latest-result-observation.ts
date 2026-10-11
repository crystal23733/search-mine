import { expect, type Page, type Route } from "@playwright/test";
import type { LatestResultResponse } from "../../../packages/protocol/src/index";

export async function observeLatestResult(
  page: Page,
  trigger: () => Promise<unknown>,
): Promise<LatestResultResponse> {
  let delivered!: (body: LatestResultResponse) => void;
  let failed!: (error: unknown) => void;
  const observed = new Promise<LatestResultResponse>((resolve, reject) => {
    delivered = resolve;
    failed = reject;
  });
  const handler = async (route: Route) => {
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
      // Observe the actual server body before delivering the same response to the app.
      await route.fulfill({ response });
      delivered(body);
    } catch (error) {
      failed(error);
      await route.abort();
    }
  };
  const url = "**/api/v1/results/latest";
  await page.route(url, handler, { times: 1 });
  // The one-use handler is removed by Playwright when matched. Keep the completed
  // response alive while the app consumes it and performs its post-response proof.
  const [, body] = await Promise.all([trigger(), observed]);
  return body;
}
