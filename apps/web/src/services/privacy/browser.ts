import type { LandingRoute } from "@liar/protocol";
import type { AuthPort } from "../auth/types";
import type { ConsentPort } from "../consent";
import type { NavigationPort } from "../navigation";
import { isLocale } from "../locale";
import type { OptionalEvents } from "./types";
export function bindOptionalLanding(
  browser: Window,
  navigation: NavigationPort,
  auth: AuthPort,
  consent: ConsentPort,
  events: OptionalEvents,
): { dispose(): void } {
  let disposed = false;
  const identity = () => `${auth.account() ?? ""}/${auth.revision()}`;
  let owner = identity();
  const visit = () => {
    events.cancel();
    if (disposed) return;
    const path = navigation.current().pathname.split("/");
    if (!isLocale(path[1])) return;
    const routes: Record<string, LandingRoute> = {
        "": "home",
        daily: "daily",
        rules: "rules",
        tutorial: "tutorial",
      },
      route = path.slice(2).join("/");
    if (Object.hasOwn(routes, route))
      void events.record(
        { type: "landing_view", route: routes[route] },
        path[1],
      );
  };
  const changed = () => {
    const next = identity();
    if (next !== owner) {
      owner = next;
      visit();
    }
  };
  const cancel = () => events.cancel();
  const restored = (event: Event) => {
    if ((event as PageTransitionEvent).persisted) cancel();
  };
  const remove = [
    navigation.subscribe(visit),
    auth.subscribe(changed),
    consent.subscribe(visit),
  ];
  for (const name of ["offline", "online", "pagehide"])
    browser.addEventListener(name, cancel);
  browser.addEventListener("pageshow", restored);
  browser.document.addEventListener("visibilitychange", cancel);
  visit();
  return {
    dispose() {
      if (disposed) return;
      disposed = true;
      cancel();
      for (const fn of remove) fn();
      for (const name of ["offline", "online", "pagehide"])
        browser.removeEventListener(name, cancel);
      browser.removeEventListener("pageshow", restored);
      browser.document.removeEventListener("visibilitychange", cancel);
    },
  };
}
