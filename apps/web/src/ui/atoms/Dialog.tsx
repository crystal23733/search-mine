import type { ComponentChildren } from "preact";
import { useEffect, useRef, useId } from "preact/hooks";
import { Button } from "./Button";
import { useUi } from "../context";
export function Dialog({
  open,
  onClose,
  title,
  children,
  focusClose = false,
}: {
  open: boolean;
  onClose(): void;
  title: string;
  children: ComponentChildren;
  focusClose?: boolean;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  const { t } = useUi();
  useEffect(() => {
    if (!open || !ref.current) return;
    const previous =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    const dialog = ref.current;
    dialog.showModal();
    if (focusClose)
      dialog
        .querySelector<HTMLButtonElement>(":scope > [data-dialog-close]")
        ?.focus();
    return () => {
      dialog.close();
      previous?.focus();
    };
  }, [open, focusClose]);
  return (
    <dialog
      ref={ref}
      class="sheet"
      aria-labelledby={titleId}
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
      onKeyDown={(event) => {
        if (event.key !== "Tab") return;
        const elements = [
          ...event.currentTarget.querySelectorAll<HTMLElement>(
            'a[href], button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])',
          ),
        ];
        const first = elements[0];
        const last = elements.at(-1);
        if (
          first &&
          last &&
          (event.shiftKey
            ? document.activeElement === first
            : document.activeElement === last)
        ) {
          event.preventDefault();
          (event.shiftKey ? last : first).focus();
        }
      }}
    >
      <h2 id={titleId}>{title}</h2>
      {children}
      <Button data-dialog-close="" variant="primary" onClick={onClose}>
        {t("close")}
      </Button>
    </dialog>
  );
}
