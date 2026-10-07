import { DEFAULT_RULES } from "@liar/protocol";
import { useUi } from "../ui/context";
import { Card } from "../ui/atoms/Card";
export function RulesPage() {
  const { t, locale } = useUi();
  const rules = DEFAULT_RULES.rules;
  const unit = (number: number, unit: "minute" | "second") =>
    new Intl.NumberFormat(locale, {
      style: "unit",
      unit,
      unitDisplay: "short",
    }).format(number);
  const data = {
    safe: rules.width * rules.height - rules.mines,
    minutes: unit(rules.duration_ms / 60_000, "minute"),
    gauge: rules.gauge_capacity,
    lies: rules.max_lies,
    mine: unit(rules.mine_stun_ms / 1000, "second"),
    reflect: unit(rules.reflect_stun_ms / 1000, "second"),
    wrong: unit(rules.wrong_accuse_stun_ms / 1000, "second"),
  };
  return (
    <div class="reading-page">
      <h1>{t("rules.title")}</h1>
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
    </div>
  );
}
