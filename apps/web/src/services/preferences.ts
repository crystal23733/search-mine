import { isLocale, type Locale } from "./locale";
export interface Preferences {
  locale?: Locale;
  motion: "system" | "reduce";
  contrast: "standard" | "high";
  zoom: 1 | 1.25 | 1.5 | 2;
}
export interface PreferencesPort {
  read(): Preferences;
  persistent(): boolean;
  update(patch: Partial<Preferences>): void;
  subscribe(listener: () => void): () => void;
}
export const DEFAULT_PREFERENCES: Preferences = {
  motion: "system",
  contrast: "standard",
  zoom: 1,
};
export function createPreferences(
  storage?: Pick<Storage, "getItem" | "setItem">,
): PreferencesPort {
  const validate = (value: Partial<Preferences>): Preferences => ({
    ...(isLocale(value.locale) ? { locale: value.locale } : {}),
    motion: value.motion === "reduce" ? "reduce" : "system",
    contrast: value.contrast === "high" ? "high" : "standard",
    zoom:
      value.zoom === 1.25 || value.zoom === 1.5 || value.zoom === 2
        ? value.zoom
        : 1,
  });
  let state = { ...DEFAULT_PREFERENCES };
  let persistent = Boolean(storage);
  let raw: string | null | undefined;
  try {
    raw = storage?.getItem("liar.preferences.v1");
  } catch {
    persistent = false;
  }
  if (raw && raw.length <= 2048) {
    try {
      const value: unknown = JSON.parse(raw);
      if (value && typeof value === "object" && !Array.isArray(value))
        state = validate(value);
    } catch {
      /* Invalid preference JSON leaves defaults without misreporting storage availability. */
    }
  }
  const listeners = new Set<() => void>();
  return {
    read: () => ({ ...state }),
    persistent: () => persistent,
    update: (patch) => {
      state = validate({ ...state, ...patch });
      try {
        storage?.setItem("liar.preferences.v1", JSON.stringify(state));
        persistent = Boolean(storage);
      } catch {
        persistent = false;
        /* Memory fallback remains available. */
      }
      for (const listener of listeners) listener();
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}
