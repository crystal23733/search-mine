import { ruleCopyData } from "../services/rule-copy";
import { useUi } from "../ui/context";
import { Card } from "../ui/atoms/Card";
import { NavLink } from "../ui/molecules/NavLink";
import { PlayBasics } from "../ui/molecules/PlayBasics";
export function RulesPage() {
  const { t, locale } = useUi();
  const data = ruleCopyData(locale);
  return (
    <div class="reading-page">
      <h1>{t("rules.title")}</h1>
      <Card>
        <PlayBasics />
      </Card>
      <Card>
        {(
          [
            "rules.win",
            "rules.gauge",
            "rules.lies",
            "rules.stun",
            "rules.flags",
          ] as const
        ).map((key) => (
          <p key={key}>{t(key, data)}</p>
        ))}
      </Card>
      <NavLink path="/tutorial" class="button button--primary">
        {t("tutorial.replay")}
      </NavLink>
    </div>
  );
}
