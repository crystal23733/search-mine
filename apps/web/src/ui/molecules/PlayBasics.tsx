import { useEffect, useState } from "preact/hooks";
import { useUi } from "../context";

export function PlayBasics({ collapse = false }: { collapse?: boolean }) {
  const { t } = useUi();
  const [expanded, setExpanded] = useState(!collapse);
  useEffect(() => {
    if (collapse) setExpanded(false);
  }, [collapse]);
  return (
    <details
      class="play-basics"
      open={expanded}
      onToggle={(event) => setExpanded(event.currentTarget.open)}
    >
      <summary>{t("basics.title")}</summary>
      <p>{t("basics.goal")}</p>
      <dl>
        {(["numbers", "flags", "controls", "lies"] as const).map((topic) => (
          <div key={topic}>
            <dt>{t(`basics.${topic}.title`)}</dt>
            <dd>
              {t(`basics.${topic}`, {
                open: t("board.open"),
                flag: t("board.flag"),
                accuse: t("board.accuse"),
              })}
            </dd>
          </div>
        ))}
      </dl>
    </details>
  );
}
