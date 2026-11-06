import { useUi } from "../ui/context";
import { Card } from "../ui/atoms/Card";
import { PlayBasics } from "../ui/molecules/PlayBasics";
import { GUIDE_STEPS } from "../services/tutorial-guide";
export function TutorialGuidePage() {
  const { t, locale } = useUi();
  return (
    <div class="reading-page">
      <h1>{t("learn.title")}</h1>
      <p>{t("learn.description")}</p>
      <Card>
        <h2>{t("learn.steps")}</h2>
        <ol>
          {GUIDE_STEPS.map((step) => (
            <li key={step}>
              <h3>{t(`home.${step}`)}</h3>
              <p>{t(`home.${step}Hint`)}</p>
            </li>
          ))}
        </ol>
      </Card>
      <Card>
        <PlayBasics />
      </Card>
      <p>{t("learn.ready")}</p>
      <div class="match-controls">
        <a href={`/${locale}/tutorial`} class="button button--primary">
          {t("learn.start")}
        </a>
        <a href={`/${locale}/rules`} class="button">
          {t("rules.title")}
        </a>
      </div>
    </div>
  );
}
