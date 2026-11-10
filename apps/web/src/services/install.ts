export interface InstallState {
  status:
    | "guide"
    | "available"
    | "prompting"
    | "requested"
    | "installed"
    | "standalone";
  feedback: "none" | "dismissed" | "failed";
}
export interface InstallPort {
  read(): InstallState;
  subscribe(listener: () => void): () => void;
  install(): Promise<boolean>;
  dispose(): void;
}
interface Invitation extends Event {
  prompt(): Promise<unknown>;
  userChoice: Promise<{ outcome: string }>;
}
export function createInstall(win?: Window): InstallPort {
  let state: InstallState = { status: "guide", feedback: "none" };
  let invitation: Invitation | undefined;
  let media: MediaQueryList | undefined;
  let disposed = false,
    installed = false,
    working = false,
    generation = 0;
  let requested = false;
  const seen = new WeakSet<Event>();
  const listeners = new Set<() => void>();
  try {
    media = win?.matchMedia("(display-mode: standalone)");
  } catch {
    /* Menu instructions remain available. */
  }
  const standalone = () =>
    Boolean(
      media?.matches ||
      (win?.navigator as (Navigator & { standalone?: boolean }) | undefined)
        ?.standalone === true,
    );
  const publish = (feedback: InstallState["feedback"] = "none") => {
    if (disposed) return;
    state = {
      status: standalone()
        ? "standalone"
        : installed
          ? "installed"
          : working
            ? "prompting"
            : invitation
              ? "available"
              : requested
                ? "requested"
                : "guide",
      feedback,
    };
    listeners.forEach((listener) => listener());
  };
  const beforeInstall = (event: Event) => {
    if (disposed || installed || standalone() || seen.has(event)) return;
    const candidate = event as Partial<Invitation>;
    if (
      typeof candidate.prompt !== "function" ||
      typeof candidate.userChoice?.then !== "function"
    )
      return;
    seen.add(event);
    event.preventDefault();
    if (working) return;
    invitation = candidate as Invitation;
    requested = false;
    publish();
  };
  const complete = () => {
    if (disposed) return;
    generation++;
    installed = true;
    invitation = undefined;
    working = false;
    publish();
  };
  const displayChange = () => {
    if (standalone()) {
      generation++;
      invitation = undefined;
      working = false;
    }
    publish();
  };
  win?.addEventListener("beforeinstallprompt", beforeInstall);
  win?.addEventListener("appinstalled", complete);
  media?.addEventListener("change", displayChange);
  publish();
  return {
    read: () => ({ ...state }),
    subscribe: (listener) => {
      if (disposed) return () => {};
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    install: async () => {
      if (disposed || working || installed || standalone() || !invitation)
        return false;
      const current = invitation,
        token = ++generation;
      invitation = undefined;
      working = true;
      requested = false;
      publish();
      try {
        // Keep prompt() in the original user activation, before the first await.
        await current.prompt();
        const choice = await current.userChoice;
        if (disposed || token !== generation) return false;
        working = false;
        if (choice.outcome === "accepted") {
          requested = true;
          publish();
          return true;
        }
        publish(choice.outcome === "dismissed" ? "dismissed" : "failed");
      } catch {
        if (!disposed && token === generation) {
          working = false;
          publish("failed");
        }
      }
      return false;
    },
    dispose: () => {
      if (disposed) return;
      disposed = true;
      generation++;
      invitation = undefined;
      working = false;
      listeners.clear();
      win?.removeEventListener("beforeinstallprompt", beforeInstall);
      win?.removeEventListener("appinstalled", complete);
      media?.removeEventListener("change", displayChange);
    },
  };
}
