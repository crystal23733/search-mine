import { useUi } from "../ui/context";
import { useSnapshot } from "../ui/useSnapshot";
import { ConsentSettings } from "../ui/organisms/ConsentSettings";
import { ServerConsentControls } from "../ui/organisms/ServerConsentControls";
import { NavLink } from "../ui/molecules/NavLink";
export function ConsentPage() {
  const { services, t } = useUi();
  const auth = useSnapshot(services.auth);
  return (
    <div class="reading-page">
      <h1>{t("consent.pageTitle")}</h1>
      <p>{t("consent.pageIntro")}</p>
      <ConsentSettings />
      {auth.status === "ready" && auth.account ? (
        <ServerConsentControls />
      ) : (
        <p>{t("consent.accountOptional")}</p>
      )}
      <NavLink path="/storage">{t("storage.pageTitle")}</NavLink>
    </div>
  );
}
