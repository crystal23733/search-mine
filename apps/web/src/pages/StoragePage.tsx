import { useUi } from "../ui/context";
import { Card } from "../ui/atoms/Card";
import { NavLink } from "../ui/molecules/NavLink";
import { StorageNotice } from "../ui/organisms/StorageNotice";

export function StoragePage() {
  const { t } = useUi();
  return (
    <div class="reading-page">
      <h1>{t("storage.pageTitle")}</h1>
      <Card>
        <StorageNotice />
      </Card>
      <NavLink path="/settings" class="button">
        {t("settings")}
      </NavLink>
    </div>
  );
}
