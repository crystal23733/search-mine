import { expect, test, vi } from "vitest";
import { bindOptionalLanding } from "./browser";
import { createNavigation } from "../navigation";
import { createConsent } from "../consent";
import { authTestPorts, TEST_ACCOUNT } from "../../../test/auth-ports";
test("only public landing paths produce closed route names, with cancellation before navigation and lifecycle changes", async () => {
  window.history.replaceState(null, "", "/ko/?invite=private#secret");
  const f = authTestPorts(TEST_ACCOUNT, true);
  await f.auth.refresh();
  const consent = createConsent();
  const events = {
    record: vi.fn().mockResolvedValue(false),
    cancel: vi.fn(),
    dispose: vi.fn(),
  };
  const nav = createNavigation(window);
  const binding = bindOptionalLanding(window, nav, f.auth, consent, events);
  expect(events.record).toHaveBeenLastCalledWith(
    { type: "landing_view", route: "home" },
    "ko",
  );
  nav.go("/rules");
  expect(events.record).toHaveBeenLastCalledWith(
    { type: "landing_view", route: "rules" },
    "ko",
  );
  nav.go("/me/privacy");
  expect(events.record).toHaveBeenCalledTimes(2);
  const before = events.cancel.mock.calls.length;
  window.dispatchEvent(new Event("offline"));
  document.dispatchEvent(new Event("visibilitychange"));
  window.dispatchEvent(new Event("pagehide"));
  expect(events.cancel.mock.calls.length).toBe(before + 3);
  binding.dispose();
  const stopped = events.cancel.mock.calls.length;
  nav.go("/daily");
  window.dispatchEvent(new Event("online"));
  expect(events.cancel).toHaveBeenCalledTimes(stopped);
  expect(events.record).toHaveBeenCalledTimes(2);
});
