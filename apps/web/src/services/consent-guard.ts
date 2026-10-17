import type { CmpPort } from "./consent";
export interface ConsentContextPort {
  capture(): object | null;
  subscribe(listener: () => void): () => void;
  invalidate(): void;
}
export interface ConsentClock {
  wall: number;
  monotonic: number;
}
export interface ConsentReceipt {
  status: "valid";
  context: object;
  revision: number;
  validFrom: number;
  expiresAt: number;
  analytics: boolean;
  advertising: boolean;
}
export function createConsentGuard(
  cmp: CmpPort,
  context: ConsentContextPort,
  clock: () => ConsentClock = () => ({
    wall: Date.now(),
    monotonic: performance.now(),
  }),
): CmpPort & { dispose(): void } {
  let disposed = false,
    broken = false,
    clockBroken = false;
  let token: object | null | undefined;
  let highest = 0,
    retired = 0,
    deadline = 0;
  let previous: ConsentReceipt | undefined, lastTime: ConsentClock | undefined;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let active: { cancel(): void } | undefined;
  const listeners = new Set<() => void>();
  const removers: Array<() => void> = [];
  const emit = () => {
    if (!disposed) for (const listener of listeners) listener();
  };
  const clear = () => {
    clearTimeout(timer);
    timer = undefined;
  };
  const stop = (status: "unknown" | "denied" | "error") => {
    retired = highest;
    clear();
    return { status };
  };
  function current() {
    const next = context.capture();
    if (next !== token) {
      token = next;
      highest = retired = 0;
      previous = lastTime = undefined;
      clockBroken = false;
      clear();
      active?.cancel();
    }
    return next;
  }
  function read() {
    if (disposed || broken) return { status: "error" };
    try {
      const owner = current();
      if (!owner || active) return stop("unknown");
      const raw = cmp.read();
      if (!raw || typeof raw !== "object" || Array.isArray(raw))
        return stop("error");
      const value = raw as Record<string, unknown>;
      const keys = Object.keys(value);
      if (
        keys.length === 1 &&
        (value.status === "unknown" ||
          value.status === "denied" ||
          value.status === "error")
      )
        return stop(value.status);
      const {
        status,
        context: captured,
        revision,
        validFrom,
        expiresAt,
        analytics,
        advertising,
      } = value;
      if (
        status !== "valid" ||
        keys.length !== 7 ||
        !keys.every((key) =>
          [
            "status",
            "context",
            "revision",
            "validFrom",
            "expiresAt",
            "analytics",
            "advertising",
          ].includes(key),
        ) ||
        typeof revision !== "number" ||
        !Number.isSafeInteger(revision) ||
        revision <= 0 ||
        typeof validFrom !== "number" ||
        !Number.isSafeInteger(validFrom) ||
        validFrom < 0 ||
        typeof expiresAt !== "number" ||
        !Number.isSafeInteger(expiresAt) ||
        expiresAt > 8640000000000000 ||
        expiresAt <= validFrom ||
        typeof analytics !== "boolean" ||
        typeof advertising !== "boolean"
      )
        return stop("error");
      if (current() !== owner) return stop("unknown");
      const sampled = clock();
      const time = { wall: sampled.wall, monotonic: sampled.monotonic };
      if (
        !Number.isSafeInteger(time.wall) ||
        time.wall < 0 ||
        time.wall > 8640000000000000 ||
        !Number.isFinite(time.monotonic) ||
        time.monotonic < 0 ||
        time.monotonic > Number.MAX_SAFE_INTEGER ||
        (lastTime &&
          (time.wall < lastTime.wall || time.monotonic < lastTime.monotonic))
      )
        clockBroken = true;
      if (clockBroken) return stop("error");
      lastTime = time;
      if (
        current() !== owner ||
        captured !== owner ||
        revision < highest ||
        revision <= retired
      )
        return stop("unknown");
      const receipt: ConsentReceipt = {
        status: "valid",
        context: owner,
        revision,
        validFrom,
        expiresAt,
        analytics,
        advertising,
      };
      if (
        revision === highest &&
        previous &&
        (previous.validFrom !== validFrom ||
          previous.expiresAt !== expiresAt ||
          previous.analytics !== analytics ||
          previous.advertising !== advertising)
      )
        return stop("error");
      if (revision > highest) {
        clear();
        highest = revision;
        previous = receipt;
        deadline = time.monotonic + expiresAt - time.wall;
      }
      if (
        time.wall < validFrom ||
        time.wall >= expiresAt ||
        time.monotonic >= deadline
      )
        return stop("unknown");
      if (!Number.isFinite(deadline) || deadline > Number.MAX_SAFE_INTEGER)
        return stop("error");
      if (!timer)
        timer = setTimeout(
          () => {
            timer = undefined;
            read();
            emit();
          },
          Math.min(
            2147483647,
            expiresAt - time.wall,
            deadline - time.monotonic,
          ),
        );
      return { status: "valid", analytics, advertising };
    } catch {
      active?.cancel();
      return stop("error");
    }
  }
  try {
    for (const attach of [
      () =>
        context.subscribe(() => {
          try {
            current();
          } catch {
            broken = true;
            active?.cancel();
            clear();
          }
          emit();
        }),
      () =>
        cmp.subscribe(() => {
          read();
          emit();
        }),
    ]) {
      const remove = attach();
      if (typeof remove !== "function") throw Error("unavailable");
      removers.push(remove);
    }
  } catch {
    broken = true;
    clear();
    for (const remove of removers.splice(0))
      try {
        remove();
      } catch {
        /* Failed setup stays closed. */
      }
  }
  return {
    read,
    subscribe(listener) {
      if (broken) throw Error("unavailable");
      if (!disposed) listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    async reopen(signal) {
      if (disposed || broken || active || signal.aborted)
        throw Error("unavailable");
      context.invalidate();
      const owner = current();
      if (!owner) throw Error("unavailable");
      const controller = new AbortController();
      let rejectAbort!: (error: Error) => void;
      const interrupted = new Promise<never>((_, reject) => {
        rejectAbort = reject;
      });
      const request = {
        cancel() {
          controller.abort();
          rejectAbort(Error("cancelled"));
        },
      };
      active = request;
      signal.addEventListener("abort", request.cancel, { once: true });
      try {
        const operation = Promise.resolve().then(() => {
          if (controller.signal.aborted || current() !== owner)
            throw Error("cancelled");
          return cmp.reopen(controller.signal);
        });
        await Promise.race([operation, interrupted]);
        if (controller.signal.aborted || current() !== owner)
          throw Error("cancelled");
      } finally {
        signal.removeEventListener("abort", request.cancel);
        if (active === request) active = undefined;
        read();
        emit();
      }
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      active?.cancel();
      clear();
      listeners.clear();
      for (const remove of removers)
        try {
          remove();
        } catch {
          /* Disposal stays closed. */
        }
    },
  };
}
