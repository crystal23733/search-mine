import type { ComponentChildren } from "preact";
export function Card({
  children,
  class: className = "",
}: {
  children: ComponentChildren;
  class?: string;
}) {
  return <section class={`card ${className}`}>{children}</section>;
}
