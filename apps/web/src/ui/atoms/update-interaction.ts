import type { ActivityPort } from "../../services/activity";
import type { UpdateInteractionPort } from "../../services/update-interaction";
export function createUpdateInteraction(
  activity: ActivityPort,
  doc: Document,
  label: () => string,
): UpdateInteractionPort & { dispose(): void } {
  let dialog: HTMLDialogElement | undefined;
  let previous: HTMLElement | undefined;
  let disposed = false;
  const close = () => {
    if (!dialog) return;
    const restore =
      doc.activeElement === doc.body || dialog.contains(doc.activeElement);
    if (dialog.open) dialog.close();
    dialog.remove();
    dialog = undefined;
    if (restore && previous?.isConnected)
      previous.focus({ preventScroll: true });
    previous = undefined;
  };
  const sync = () => {
    if (disposed) return;
    if (!activity.read().locked) {
      close();
      return;
    }
    if (dialog?.open) return;
    const current = doc.activeElement;
    previous = current instanceof HTMLElement ? current : undefined;
    dialog = doc.createElement("dialog");
    dialog.className = "update-interaction";
    dialog.setAttribute("aria-label", label());
    dialog.tabIndex = -1;
    const output = doc.createElement("output");
    output.textContent = label();
    dialog.append(output);
    dialog.addEventListener("cancel", (event) => event.preventDefault());
    doc.body.append(dialog);
    try {
      dialog.showModal();
      dialog.focus({ preventScroll: true });
    } catch {
      close();
    }
  };
  const unsubscribe = activity.subscribe(sync);
  sync();
  return {
    ready: () =>
      !disposed &&
      Boolean(dialog?.open && dialog.isConnected && activity.read().locked),
    dispose: () => {
      if (disposed) return;
      disposed = true;
      unsubscribe();
      close();
    },
  };
}
