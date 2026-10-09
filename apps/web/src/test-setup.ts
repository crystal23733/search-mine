import { afterEach } from "vitest";
if (typeof document !== "undefined") {
  const { cleanup } = await import("@testing-library/preact");
  afterEach(cleanup);
  // jsdom has no native modal API; actual focus behavior is tested in Chromium.
  HTMLDialogElement.prototype.showModal = function () {
    this.open = true;
  };
  HTMLDialogElement.prototype.close = function () {
    this.open = false;
  };
}
