import { useEffect, useState } from "preact/hooks";
import { useUi } from "../ui/context";
import { Card } from "../ui/atoms/Card";
import { Button } from "../ui/atoms/Button";
import { Dialog } from "../ui/atoms/Dialog";
import { LanguageSelect } from "../ui/molecules/LanguageSelect";
import { AccountSettings } from "../ui/organisms/AccountSettings";
import type { Preferences } from "../services/preferences";
import { useSnapshot } from "../ui/useSnapshot";
export function SettingsPage() {
  const { t, locale, preferences, services } = useUi();
  const [notice, setNotice] = useState(false);
  const offline = useSnapshot(services.offline);
  const activity = useSnapshot(services.activity);
  const [pending, setPending] = useState(0);
  const [result, setResult] = useState<"idle" | "cleared" | "error">("idle");
  const [confirm, setConfirm] = useState(false);
  useEffect(() => {
    let active = true;
    void services.dailyRecords
      .pending()
      .then((value) => {
        if (active) setPending(value.value.length);
      })
      .catch(() => {
        if (active) setResult("error");
      });
    return () => {
      active = false;
    };
  }, [services]);
  return (
    <div class="reading-page">
      <h1>{t("settings")}</h1>
      <AccountSettings />
      <h2>{t("offline.title")}</h2>
      <Card>
        <p>
          {t(
            offline.disabled
              ? "offline.disabled"
              : offline.ready
                ? "offline.cached"
                : "offline.notCached",
          )}
        </p>
        <p>{t("offline.pending", { count: pending })}</p>
        <p class="muted">{t("offline.pendingHint")}</p>
        <div class="match-controls">
          <Button
            disabled={
              offline.working ||
              !offline.ready ||
              activity.busy ||
              activity.locked
            }
            onClick={() => {
              void services.offline.clear();
            }}
          >
            {t("offline.clearCache")}
          </Button>
          <Button
            disabled={
              offline.working ||
              !offline.online ||
              (!offline.disabled && offline.ready)
            }
            onClick={() => {
              void services.offline.enable();
            }}
          >
            {t("offline.enableCache")}
          </Button>
          <Button
            variant="ghost"
            disabled={activity.busy || activity.locked}
            onClick={() => setConfirm(true)}
          >
            {t("offline.clearRecords")}
          </Button>
        </div>
        {result !== "idle" && (
          <output>
            {t(result === "cleared" ? "offline.cleared" : "offline.error")}
          </output>
        )}
      </Card>
      <h2>{t("settings.accessibility")}</h2>
      <Card class="settings-list">
        <label>
          {t("language")}
          <LanguageSelect />
        </label>
        <label>
          {t("settings.motion")}
          <input
            type="checkbox"
            checked={preferences.motion === "reduce"}
            onChange={(event) =>
              services.preferences.update({
                motion: event.currentTarget.checked ? "reduce" : "system",
              })
            }
          />
        </label>
        <label>
          {t("settings.contrast")}
          <input
            type="checkbox"
            checked={preferences.contrast === "high"}
            onChange={(event) =>
              services.preferences.update({
                contrast: event.currentTarget.checked ? "high" : "standard",
              })
            }
          />
        </label>
        <label>
          {t("settings.zoom")}
          <select
            value={preferences.zoom}
            onChange={(event) =>
              services.preferences.update({
                zoom: Number(event.currentTarget.value) as Preferences["zoom"],
              })
            }
          >
            {[1, 1.25, 1.5, 2].map((zoom) => (
              <option key={zoom} value={zoom}>
                {new Intl.NumberFormat(locale, { style: "percent" }).format(
                  zoom,
                )}
              </option>
            ))}
          </select>
        </label>
        <Button variant="ghost" onClick={() => setNotice(true)}>
          {t("storage.title")}
        </Button>
      </Card>
      <output class="muted preferences-status">
        {t(
          services.preferences.persistent()
            ? "settings.saved"
            : "settings.memory",
        )}
      </output>
      <Dialog
        open={confirm}
        onClose={() => setConfirm(false)}
        title={t("offline.clearRecords")}
      >
        <p>{t("offline.clearConfirm")}</p>
        <Button
          disabled={activity.busy || activity.locked}
          onClick={() => {
            if (
              services.activity.read().busy ||
              services.activity.read().locked
            ) {
              setResult("error");
              return;
            }
            setConfirm(false);
            const release = services.activity.hold();
            void services.dailyRecords
              .clear()
              .then((value) => {
                setPending(0);
                setResult(value.warning ? "error" : "cleared");
              })
              .catch(() => setResult("error"))
              .finally(release);
          }}
        >
          {t("offline.confirm")}
        </Button>
      </Dialog>
      <Dialog
        open={notice}
        onClose={() => setNotice(false)}
        title={t("storage.title")}
      >
        <p>{t("storage.description")}</p>
      </Dialog>
    </div>
  );
}
