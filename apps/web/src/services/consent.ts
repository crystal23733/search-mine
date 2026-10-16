export type ConsentStatus =
  "unavailable" | "unknown" | "denied" | "error" | "valid";
export interface ConsentView {
  status: ConsentStatus;
  analytics: boolean;
  advertising: boolean;
  working: boolean;
  canReopen: boolean;
}
export interface CmpPort {
  read(): unknown;
  subscribe(listener: () => void): () => void;
  reopen(signal: AbortSignal): Promise<void>;
}
export interface ConsentPort {
  read(): ConsentView;
  subscribe(listener: () => void): () => void;
  reopen(signal?: AbortSignal): Promise<boolean>;
  dispose(): void;
}
function blocked(status: ConsentStatus) {
  return { status, analytics: false, advertising: false };
}
function choiceView(choice: unknown) {
  if (!choice || typeof choice !== "object" || Array.isArray(choice))
    return blocked("error");
  const value = choice as Record<string, unknown>;
  const keys = Object.keys(value);
  if (
    value.status === "valid" &&
    keys.length === 3 &&
    keys.every((key) => ["status", "analytics", "advertising"].includes(key)) &&
    typeof value.analytics === "boolean" &&
    typeof value.advertising === "boolean"
  ) {
    return {
      status: "valid" as const,
      analytics: value.analytics,
      advertising: value.advertising,
    };
  }
  if (
    keys.length === 1 &&
    (value.status === "unknown" ||
      value.status === "denied" ||
      value.status === "error")
  )
    return blocked(value.status);
  return blocked("error");
}
export function createConsent(cmp?: CmpPort): ConsentPort {
  const listeners = new Set<() => void>();
  let disposed = false;
  let broken = false;
  let failed = false;
  let pending: { cancel(): void } | undefined;
  let unsubscribe = () => {};
  const emit = () => {
    if (!disposed) for (const listener of listeners) listener();
  };
  if (cmp)
    try {
      unsubscribe = cmp.subscribe(() => {
        failed = false;
        emit();
      });
      if (typeof unsubscribe !== "function") broken = true;
    } catch {
      broken = true;
    }
  return {
    read() {
      const metadata = {
        working: Boolean(pending),
        canReopen: Boolean(cmp) && !disposed && !broken,
      };
      if (!cmp || disposed) return { ...blocked("unavailable"), ...metadata };
      if (broken || failed) return { ...blocked("error"), ...metadata };
      if (pending) return { ...blocked("unknown"), ...metadata };
      try {
        return { ...choiceView(cmp.read()), ...metadata };
      } catch {
        return { ...blocked("error"), ...metadata };
      }
    },
    subscribe(listener) {
      if (!disposed) listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    reopen(signal) {
      if (!cmp || disposed || broken || pending || signal?.aborted)
        return Promise.resolve(false);
      const adapter = cmp;
      return new Promise<boolean>((resolve) => {
        const controller = new AbortController();
        let settled = false;
        const finish = (ok: boolean, error: boolean) => {
          if (settled) return;
          settled = true;
          signal?.removeEventListener("abort", cancel);
          pending = undefined;
          failed = error;
          emit();
          resolve(ok);
        };
        const cancel = () => {
          controller.abort();
          finish(false, false);
        };
        pending = { cancel };
        failed = false;
        signal?.addEventListener("abort", cancel, { once: true });
        emit();
        if (signal?.aborted) cancel();
        void Promise.resolve()
          .then(() => {
            if (!controller.signal.aborted)
              return adapter.reopen(controller.signal);
          })
          .then(
            () => finish(true, false),
            () => finish(false, !controller.signal.aborted),
          );
      });
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      pending?.cancel();
      listeners.clear();
      try {
        unsubscribe();
      } catch {
        /* The disposed gate stays closed even if adapter cleanup fails. */
      }
    },
  };
}
