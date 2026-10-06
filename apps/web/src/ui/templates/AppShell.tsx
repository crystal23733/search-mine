import type { ComponentChildren } from "preact";
import { useUi } from "../context";
import { Header } from "../organisms/Header";
import { NavLink } from "../molecules/NavLink";
export function AppShell({ children }: { children: ComponentChildren }) {
  const { t } = useUi();
  return (
    <div class="app-shell">
      <a class="skip-link" href="#main">
        {t("skip")}
      </a>
      <Header />
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
