import type { JSX } from "preact";
export function Button({
  variant = "secondary",
  class: className = "",
  ...props
}: JSX.IntrinsicElements["button"] & {
  variant?: "primary" | "secondary" | "ghost";
}) {
  return (
    <button
      type="button"
      class={`button button--${variant} ${className}`}
      {...props}
    />
  );
}
