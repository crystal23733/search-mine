import { useUi } from "../context";
import { NavLink } from "../molecules/NavLink";
import { LanguageSelect } from "../molecules/LanguageSelect";
import { Icon } from "../atoms/Icon";
export function Header() {
  const { t } = useUi();
  return (
    <header class="header">
      <NavLink path="/" class="brand" label="Liar Sweeper">
        <span class="brand-square" aria-hidden="true">
          4
        </span>
        <span>
          Liar <span class="accent">Sweeper</span>
        </span>
      </NavLink>
      <nav class="desktop-nav" aria-label={t("home")}>
        <NavLink path="/daily">{t("home.daily")}</NavLink>
        <NavLink path="/rules">{t("home.learn")}</NavLink>
      </nav>
      <div class="header-controls">
        <span class="language-control">
          <Icon name="globe" />
          <LanguageSelect />
        </span>
        <NavLink
          path="/settings"
          class="button icon-button"
          label={t("settings")}
        >
          <Icon name="settings" />
        </NavLink>
      </div>
    </header>
  );
}
