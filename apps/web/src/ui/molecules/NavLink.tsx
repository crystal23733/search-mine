import type { ComponentChildren } from "preact";
import { useUi } from "../context";
export function NavLink({
  path,
  children,
  class: className = "",
  label,
  query,
}: {
  path: string;
  children: ComponentChildren;
  class?: string;
  label?: string;
  query?: Record<string, string | null>;
}) {
  const { locale, services } = useUi();
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query ?? {}))
    if (value !== null) params.set(key, value);
  return (
    <a
      href={`/${locale}${path}${params.size ? `?${params}` : ""}`}
      class={className}
      aria-label={label}
      onClick={(event) => {
        if (
          event.button !== 0 ||
          event.metaKey ||
          event.ctrlKey ||
          event.shiftKey ||
          event.altKey
        )
          return;
        event.preventDefault();
        services.navigation.go(path, query);
      }}
    >
      {children}
    </a>
  );
}
