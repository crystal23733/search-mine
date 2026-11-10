import { expect, test, vi } from "vitest";
import { createActivity } from "../../services/activity";
import { createUpdateInteraction } from "./update-interaction";
test("active games remain interactive and idle preparation opens a synchronous accessible modal before acknowledgement", () => {
  const button = document.createElement("button");
  document.body.append(button);
  button.focus();
  const activity = createActivity(),
    guard = createUpdateInteraction(
      activity,
      document,
      () => "Applying changes",
    );
  try {
    const finish = activity.hold();
    expect(activity.prepare("busy")).toBe(false);
    expect(document.querySelector("dialog")).toBeNull();
    expect(guard.ready()).toBe(false);
    finish();
    expect(activity.prepare("idle")).toBe(true);
    const dialog = document.querySelector("dialog")!;
    expect(dialog?.open).toBe(true);
    expect(guard.ready()).toBe(true);
    expect(dialog.getAttribute("aria-label")).toBe("Applying changes");
    expect(dialog.textContent).toBe("Applying changes");
    const cancel = new Event("cancel", { cancelable: true });
    dialog.dispatchEvent(cancel);
    expect(cancel.defaultPrevented).toBe(true);
    activity.release("foreign");
    expect(guard.ready()).toBe(true);
    activity.release("idle");
    expect(guard.ready()).toBe(false);
    expect(document.querySelector("dialog")).toBeNull();
    expect(document.activeElement).toBe(button);
  } finally {
    guard.dispose();
    activity.release("idle");
    button.remove();
  }
});
test("the original lease expiry and adapter disposal remove the modal without leaving input locked", async () => {
  vi.useFakeTimers();
  const activity = createActivity(),
    guard = createUpdateInteraction(
      activity,
      document,
      () => "Applying changes",
    );
  try {
    activity.prepare("lost");
    expect(guard.ready()).toBe(true);
    await vi.advanceTimersByTimeAsync(15000);
    expect(activity.read().locked).toBe(false);
    expect(document.querySelector("dialog")).toBeNull();
    activity.prepare("later");
    expect(guard.ready()).toBe(true);
    guard.dispose();
    guard.dispose();
    expect(document.querySelector("dialog")).toBeNull();
    expect(guard.ready()).toBe(false);
    activity.release("later");
    activity.prepare("after");
    expect(document.querySelector("dialog")).toBeNull();
  } finally {
    guard.dispose();
    activity.release("after");
    activity.release("later");
    vi.useRealTimers();
  }
});
test("unavailable modal APIs fail closed and remove partial dialog DOM", () => {
  const activity = createActivity();
  const show = vi
    .spyOn(HTMLDialogElement.prototype, "showModal")
    .mockImplementation(() => {
      throw Error("denied");
    });
  const guard = createUpdateInteraction(
    activity,
    document,
    () => "Applying changes",
  );
  try {
    activity.prepare("unsupported");
    expect(guard.ready()).toBe(false);
    expect(document.querySelector("dialog")).toBeNull();
  } finally {
    activity.release("unsupported");
    guard.dispose();
    show.mockRestore();
  }
});
