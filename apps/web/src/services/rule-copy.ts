import { DEFAULT_RULES } from "@liar/protocol";
import type { Locale } from "./locale.ts";
export function ruleCopyData(locale: Locale) {
  const rules = DEFAULT_RULES.rules;
  const unit = (value: number, unit: "minute" | "second") =>
    new Intl.NumberFormat(locale, {
      style: "unit",
      unit,
      unitDisplay: "short",
    }).format(value);
  return {
    safe: rules.width * rules.height - rules.mines,
    minutes: unit(rules.duration_ms / 60000, "minute"),
    gauge: rules.gauge_capacity,
    lies: rules.max_lies,
    mine: unit(rules.mine_stun_ms / 1000, "second"),
    reflect: unit(rules.reflect_stun_ms / 1000, "second"),
    wrong: unit(rules.wrong_accuse_stun_ms / 1000, "second"),
  };
}
