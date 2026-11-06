import { isLocale } from "./locale.ts";
export const DOCUMENT_ROUTES = [
  "/",
  "/practice",
  "/tutorial",
  "/learn",
  "/daily",
  "/daily/rankings",
  "/daily/official",
  "/queue",
  "/friends",
  "/results",
  "/me",
  "/me/history",
  "/me/account",
  "/me/privacy",
  "/settings",
  "/storage",
  "/login",
  "/onboarding",
  "/rules",
] as const;
export function documentRoute(pathname: string): string {
  const parts = pathname.split("/");
  if (isLocale(parts[1])) parts.splice(1, 1);
  return parts.join("/") || "/";
}
export function documentStatus(pathname: string): 200 | 403 | 404 {
  const route = documentRoute(pathname);
  if (route === "/403") return 403;
  return DOCUMENT_ROUTES.some((known) => known === route) ? 200 : 404;
}
export function isApplicationDocument(pathname: string): boolean {
  return (
    !/^\/(?:api|ws|health|assets|src|node_modules|@vite|@id|@fs)(?:\/|$)/.test(
      pathname,
    ) &&
    !/^\/(?:service-worker\.js|registerSW\.js|favicon\.[^/]+|manifest\.[^/]+)$/.test(
      pathname,
    )
  );
}
