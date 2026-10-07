import { cleanup } from "@testing-library/preact";
import { afterEach } from "vitest";
afterEach(cleanup);
// jsdom has no native modal API; actual focus behavior is tested in Chromium.
HTMLDialogElement.prototype.showModal = function () {
  this.open = true;
};
HTMLDialogElement.prototype.close = function () {
  this.open = false;
};
