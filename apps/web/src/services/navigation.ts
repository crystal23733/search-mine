import { isLocale, localizedPath, type Locale } from "./locale";
export interface NavigationPort {
  current(): URL;
  go(path: string, query?: Record<string, string | null>): void;
  locale(locale: Locale): void;
  subscribe(listener: () => void): () => void;
}
export function createNavigation(browser: Window): NavigationPort {
  const listeners = new Set<() => void>();
  const publish = () => {
    for (const listener of listeners) listener();
  };
  const update = (url: URL) => {
    browser.history.pushState(null, "", url);
    publish();
  };
  browser.addEventListener("popstate", publish);
  return {
    current: () => new URL(browser.location.href),
    go: (path, query) => {
      const url = new URL(browser.location.href);
      const prefix = url.pathname.split("/")[1];
      url.pathname = `/${isLocale(prefix) ? prefix : "en"}${path}`;
      for (const [key, value] of Object.entries(query ?? {})) {
        if (value === null) url.searchParams.delete(key);
        else url.searchParams.set(key, value);
      }
      update(url);
    },
    locale: (locale) => {
      const url = new URL(browser.location.href);
      url.pathname = localizedPath(url.pathname, locale);
      update(url);
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}
