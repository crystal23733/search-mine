import { useUi } from "../context";
import { NavLink } from "../molecules/NavLink";
import { Icon } from "../atoms/Icon";
import { useSnapshot } from "../useSnapshot";
export function AccountCard() {
  const { t, services } = useUi();
  const auth = useSnapshot(services.auth);
  const account = auth.account;
  return (
    <NavLink
      path={
        account ? (account.nickname ? "/settings" : "/onboarding") : "/login"
      }
      class="account-card card"
    >
      <span class="avatar">
        <Icon name="users" />
      </span>
      <span>
        <strong>
          {account?.nickname ??
            t(account ? "auth.nicknameUnset" : "home.account")}
        </strong>
        <span class="muted account-hint">
          {t(
            auth.status === "loading"
              ? "auth.loading"
              : account
                ? "auth.account"
                : "home.accountHint",
          )}
        </span>
      </span>
      <Icon name="arrow" />
    </NavLink>
  );
}
