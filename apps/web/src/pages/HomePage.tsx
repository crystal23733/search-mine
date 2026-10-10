import { DEFAULT_RULES } from "@liar/protocol";
import { useUi } from "../ui/context";
import { Icon } from "../ui/atoms/Icon";
import { Card } from "../ui/atoms/Card";
import { NavLink } from "../ui/molecules/NavLink";
import { ModeCard } from "../ui/molecules/ModeCard";
import { AccountCard } from "../ui/organisms/AccountCard";
import { DecorativeBoard } from "../ui/organisms/DecorativeBoard";
import { useSnapshot } from "../ui/useSnapshot";
export function HomePage() {
  const { t, locale, services } = useUi();
  const auth = useSnapshot(services.auth);
  const minutes = new Intl.NumberFormat(locale, {
    style: "unit",
    unit: "minute",
    unitDisplay: "short",
  }).format(DEFAULT_RULES.rules.duration_ms / 60_000);
  return (
    <>
      <div class="mobile-account">
        <AccountCard />
      </div>
      <section class="hero">
        <div class="hero-copy">
          <p class="eyebrow">{t("home.eyebrow", { minutes })}</p>
          <h1>
            {t("home.title1")}
            <br />
            {t("home.title2")}
          </h1>
          <p class="hero-description">{t("home.description")}</p>
          <div class="hero-actions">
            <NavLink path="/queue" class="button button--primary">
              <Icon name="bolt" />
              {t("home.quick")}
              <small>{t("home.signinHint")}</small>
            </NavLink>
            <NavLink path="/friends" class="button">
              <Icon name="users" />
              {t("home.friend")}
              <small>{t("home.friendHint")}</small>
            </NavLink>
          </div>
          <div class="desktop-account">
            <AccountCard />
          </div>
          {services.auth.connected() && auth.account?.nickname && (
            <NavLink path="/results">{t("result.latest")}</NavLink>
          )}
        </div>
        <DecorativeBoard />
      </section>
      <section class="mode-grid">
        <ModeCard
          path="/daily"
          title="home.daily"
          hint="home.practiceHint"
          icon="calendar"
        />
        <ModeCard
          path="/rules"
          title="home.learn"
          hint="home.learnHint"
          icon="flag"
        />
        <div class="desktop-rule">
          <Card>
            <span class="step-number">1</span>
            <h2>{t("home.open")}</h2>
            <p>{t("home.openHint")}</p>
          </Card>
        </div>
        <div class="desktop-rule">
          <Card>
            <span class="step-number amber">2</span>
            <h2>{t("home.attack")}</h2>
            <p>{t("home.attackHint")}</p>
          </Card>
        </div>
      </section>
      <div class="home-bottom">
        <span class="muted">{t("home.practiceHint")}</span>
        <NavLink path="/practice">{t("home.practice")}</NavLink>
      </div>
    </>
  );
}
