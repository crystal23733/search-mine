import type { MessageKey } from "../../services/i18n";
import { useUi } from "../context";
import { NavLink } from "./NavLink";
import { Icon, type IconName } from "../atoms/Icon";
export function ModeCard({
  path,
  title,
  hint,
  icon,
}: {
  path: string;
  title: MessageKey;
  hint: MessageKey;
  icon: IconName;
}) {
  const { t } = useUi();
  return (
    <NavLink path={path} class="mode-card card">
      <Icon name={icon} />
      <strong>{t(title)}</strong>
      <span class="muted">{t(hint)}</span>
    </NavLink>
  );
}
