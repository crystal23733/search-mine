import { useUi } from "../ui/context";
import { Card } from "../ui/atoms/Card";
import { NavLink } from "../ui/molecules/NavLink";
export function StatusPage({ online = false }: { online?: boolean }) {
  const { t } = useUi();
  return (
    <div class="reading-page">
      <Card>
        <h1>{t(online ? "status.title" : "status.unknown")}</h1>
        <p>{t("status.description")}</p>
        <NavLink path="/" class="button button--primary">
          {t("home")}
        </NavLink>
      </Card>
    </div>
  );
}
