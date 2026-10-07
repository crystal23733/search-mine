import type { ComponentChildren } from "preact";
export function MatchLayout({
  children,
  summary,
  guide,
}: {
  children: ComponentChildren;
  summary: ComponentChildren;
  guide?: ComponentChildren;
}) {
  return (
    <div class={`match-layout ${guide ? "match-layout--guided" : ""}`}>
      {guide && <div class="match-guide">{guide}</div>}
      <div class="match-board">{children}</div>
      <aside class="match-summary">{summary}</aside>
    </div>
  );
}
