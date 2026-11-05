import { afterEach, expect, test, vi } from "vitest";
import { createNavigation } from "./navigation";
const remove: Array<() => void> = [];
afterEach(() => {
  for (const fn of remove.splice(0)) fn();
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
