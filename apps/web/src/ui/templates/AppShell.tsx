import type { ComponentChildren } from "preact";
import { useUi } from "../context";
import { Header } from "../organisms/Header";
import { NavLink } from "../molecules/NavLink";
import { OfflineNotice } from "../organisms/OfflineNotice";
export function AppShell({ children }: { children: ComponentChildren }) {
  const { t } = useUi();
  return (
    <div class="app-shell">
      <a class="skip-link" href="#main">
        {t("skip")}
      </a>
      <Header />
      <OfflineNotice />
      <main id="main" tabIndex={-1}>
        {children}
      </main>
      <footer>
        <NavLink path="/privacy">{t("privacy")}</NavLink>
        <NavLink path="/terms">{t("terms")}</NavLink>
        <NavLink path="/settings">{t("consent")}</NavLink>
      </footer>
    </div>
  );
}
