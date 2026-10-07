import { useUi } from "../context";
import { NavLink } from "../molecules/NavLink";
import { Icon } from "../atoms/Icon";
export function AccountCard() {
  const { t } = useUi();
  return (
    <NavLink path="/login" class="account-card card">
      <span class="avatar">
        <Icon name="users" />
      </span>
      <span>
        <strong>{t("home.account")}</strong>
        <span class="muted account-hint">{t("home.accountHint")}</span>
      </span>
      <Icon name="arrow" />
    </NavLink>
  );
}
