import { useUi } from "../context";
import { Card } from "../atoms/Card";

export function ReconnectNotice({
  remainingMs,
  maxMs,
  kind = "opponent",
}: {
  remainingMs: number | null;
  maxMs: number;
  kind?: "own" | "opponent";
}) {
  const { t, locale } = useUi();
  if (remainingMs === null) return null;
  const message =
    remainingMs > 0
      ? t(
          kind === "own" ? "match.reconnecting" : "match.opponentReconnecting",
          {
            seconds: new Intl.NumberFormat(locale).format(
              Math.ceil(remainingMs / 1000),
            ),
          },
        )
      : t(kind === "own" ? "queue.disconnected" : "match.reconnectWaiting");
  return (
    <div class="reconnect-notice" data-testid={`${kind}-reconnect`}>
      <Card>
        <output>{message}</output>
        <progress aria-label={message} value={remainingMs} max={maxMs} />
      </Card>
    </div>
  );
}
