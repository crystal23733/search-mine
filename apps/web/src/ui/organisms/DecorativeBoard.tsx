const pattern = [
  "xxx.....",
  "xf1.....",
  "11211...",
  "..1f1...",
  "..11211.",
  "...12f1x",
  "..12x2xx",
  "..1xxxxx",
].join("");
export function DecorativeBoard() {
  return (
    <div class="decorative-board" aria-hidden="true">
      {[...pattern].map((cell, index) => (
        <span
          key={index}
          class={`decor-cell ${cell === "x" || cell === "f" ? "closed" : ""} number-${cell}`}
        >
          {cell === "f" ? "\u2691" : /[1-8]/.test(cell) ? cell : ""}
        </span>
      ))}
    </div>
  );
}
