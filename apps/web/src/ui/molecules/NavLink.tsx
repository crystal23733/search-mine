import type { ComponentChildren } from "preact";
import { useUi } from "../context";
export function NavLink({
  path,
  children,
  class: className = "",
  label,
}: {
  path: string;
  children: ComponentChildren;
  class?: string;
  label?: string;
}) {
  const { locale, services } = useUi();
  return (
    <a
      href={`/${locale}${path}`}
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
        services.navigation.go(path);
      }}
    >
      {children}
    </a>
  );
}
