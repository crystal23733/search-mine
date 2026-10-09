import { useUi } from "../context";
import { Card } from "../atoms/Card";

export function ReconnectNotice({
  remainingMs,
  maxMs,
}: {
  remainingMs: number | null;
  maxMs: number;
}) {
  const { t, locale } = useUi();
  if (remainingMs === null) return null;
  const message =
    remainingMs > 0
      ? t("match.opponentReconnecting", {
          seconds: new Intl.NumberFormat(locale).format(
            Math.ceil(remainingMs / 1000),
          ),
        })
      : t("match.reconnectWaiting");
  return (
    <div class="reconnect-notice" data-testid="opponent-reconnect">
      <Card>
        <output>{message}</output>
        <progress aria-label={message} value={remainingMs} max={maxMs} />
      </Card>
    </div>
  );
}
