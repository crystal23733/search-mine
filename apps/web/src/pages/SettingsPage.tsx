import { useState } from "preact/hooks";
import { useUi } from "../ui/context";
import { Card } from "../ui/atoms/Card";
import { Button } from "../ui/atoms/Button";
import { Dialog } from "../ui/atoms/Dialog";
import { LanguageSelect } from "../ui/molecules/LanguageSelect";
import { AccountCard } from "../ui/organisms/AccountCard";
import type { Preferences } from "../services/preferences";
export function SettingsPage() {
  const { t, locale, preferences, services } = useUi();
  const [notice, setNotice] = useState(false);
  return (
    <div class="reading-page">
      <h1>{t("settings")}</h1>
      <AccountCard />
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
        open={notice}
        onClose={() => setNotice(false)}
        title={t("storage.title")}
      >
        <p>{t("storage.description")}</p>
      </Dialog>
    </div>
  );
}
