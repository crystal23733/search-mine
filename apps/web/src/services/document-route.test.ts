import { expect, test } from "vitest";
import {
  documentRoute,
  documentStatus,
  isApplicationDocument,
} from "./document-route";
test("document classification shares existing private and public routes without treating unknown prefixes as locales", () => {
  expect(documentRoute("/ko/me/privacy")).toBe("/me/privacy");
  expect(documentStatus("/ko/me/privacy")).toBe(200);
  expect(documentStatus("/en/daily/official")).toBe(200);
  expect(documentStatus("/fr/403")).toBe(403);
  expect(documentStatus("/de/404")).toBe(404);
  expect(documentStatus("/xx/me/privacy")).toBe(404);
  expect(documentStatus("/en/rules/extra")).toBe(404);
});
test("API, health, websocket, built assets and Vite sources remain outside HTML error handling", () => {
  for (const path of [
    "/api/v1/me/consent",
    "/ws",
    "/health/live",
    "/assets/app.js",
    "/src/main.tsx",
    "/@vite/client",
    "/service-worker.js",
  ])
    expect(isApplicationDocument(path)).toBe(false);
  expect(isApplicationDocument("/en/missing")).toBe(true);
});
