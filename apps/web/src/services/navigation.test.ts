import { afterEach, expect, test, vi } from "vitest";
import { createNavigation } from "./navigation";
const remove: Array<() => void> = [];
afterEach(() => {
  for (const fn of remove.splice(0)) fn();
});
test("multiplayer invite codes survive their onboarding flow and never reach another document", () => {
  history.replaceState(null, "", "/ko/rooms?code=ABCD2345");
  const nav = createNavigation(window);
  remove.push(nav.dispose);
  nav.go("/login", { return_path: "rooms" });
  nav.locale("fr");
  expect(nav.current().searchParams.get("code")).toBe("ABCD2345");
  nav.go("/onboarding", { return_path: "rooms" });
  nav.go("/tutorial", { return: "/rooms", return_path: null });
  nav.go("/rooms", { return: null });
  expect(nav.current().pathname).toBe("/fr/rooms");
  expect(nav.current().searchParams.get("code")).toBe("ABCD2345");
  nav.go("/");
  expect(nav.current().search).toBe("");
});
test("document guard runs before pushState and before any subscriber sees the target", () => {
  history.replaceState(null, "", "/ko/");
  let allow = false;
  const beforeChange = vi.fn(() => {
    expect(location.pathname).toBe("/ko/");
    return allow;
  });
  const nav = createNavigation(window, { beforeChange });
  remove.push(nav.dispose);
  const seen = vi.fn();
  nav.subscribe(seen);
  nav.go("/practice");
  expect(location.pathname).toBe("/ko/");
  expect(seen).not.toHaveBeenCalled();
  allow = true;
  nav.go("/rules");
  expect(location.pathname).toBe("/ko/rules");
  expect(seen).toHaveBeenCalledTimes(1);
});
test("throwing guard and guarded popstate cannot publish; disposal removes the listener", () => {
  history.replaceState(null, "", "/ko/");
  const guard = vi.fn(() => {
    throw Error("unavailable");
  });
  const nav = createNavigation(window, { beforeChange: guard });
  remove.push(nav.dispose);
  const seen = vi.fn();
  nav.subscribe(seen);
  nav.locale("fr");
  expect(location.pathname).toBe("/ko/");
  history.replaceState(null, "", "/ko/practice");
  window.dispatchEvent(new PopStateEvent("popstate"));
  expect(seen).not.toHaveBeenCalled();
  const calls = guard.mock.calls.length;
  nav.dispose();
  window.dispatchEvent(new PopStateEvent("popstate"));
  nav.go("/rules");
  expect(guard).toHaveBeenCalledTimes(calls);
  expect(seen).not.toHaveBeenCalled();
});
