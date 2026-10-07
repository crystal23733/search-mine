import { useSnapshot } from "../useSnapshot";
import { useUi } from "../context";
import { Button } from "../atoms/Button";
export function OfflineNotice() {
  const { services, t } = useUi();
  const offline = useSnapshot(services.offline);
  const activity = useSnapshot(services.activity);
  if (offline.online && !offline.waiting && !offline.error) return null;
  return (
    <aside class="offline-notice" aria-label={t("offline.title")}>
      {!offline.online && (
        <output>
          {t(offline.ready ? "offline.ready" : "offline.missing")}
        </output>
      )}
      {offline.waiting && (
        <>
          <p>{t("offline.updateHint")}</p>
          <Button
            disabled={activity.busy || activity.locked || offline.working}
            onClick={() => {
              void services.offline.update();
            }}
          >
            {t("offline.update")}
          </Button>
        </>
      )}
      {offline.error && <output>{t("offline.error")}</output>}
    </aside>
  );
}
