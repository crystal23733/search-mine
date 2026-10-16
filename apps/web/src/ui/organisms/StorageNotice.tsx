import { useUi } from "../context";

export function StorageNotice() {
  const { t } = useUi();
  const categories = [
    "preferences",
    "records",
    "cache",
    "cookies",
    "optional",
  ] as const;
  return (
    <div class="storage-notice">
      <p>{t("storage.intro")}</p>
      <dl>
        {categories.map((category) => (
          <div key={category}>
            <dt>{t(`storage.${category}.title`)}</dt>
            <dd>{t(`storage.${category}.description`)}</dd>
          </div>
        ))}
      </dl>
      <p>{t("storage.manage")}</p>
      <p>{t("storage.siteData")}</p>
      <p class="muted">{t("storage.fallback")}</p>
    </div>
  );
}
