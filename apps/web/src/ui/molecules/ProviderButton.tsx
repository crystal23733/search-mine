import type { MessageKey } from "../../services/i18n";
import { useState } from "preact/hooks";
import {
  PROVIDERS,
  type AuthIntent,
  type AuthProvider,
} from "../../services/auth/types";
import { useUi, type UiContext } from "../context";
import { useSnapshot } from "../useSnapshot";
import { Button } from "../atoms/Button";
export function providerName(
  t: UiContext["t"],
  provider: AuthProvider,
): string {
  return t(`auth.provider.${provider}` as MessageKey);
}
export function ProviderButton({
  provider,
  intent = "login",
}: {
  provider: AuthProvider;
  intent?: AuthIntent;
}) {
  const { t, locale, services } = useUi(),
    state = useSnapshot(services.auth);
  const [failed, setFailed] = useState(false);
  const available = state.providers.some(
    (p) => p.provider === provider && p.available,
  );
  const disabled = !available || state.status !== "ready" || state.working;
  const key =
    intent === "login"
      ? "auth.continue"
      : intent === "link"
        ? "auth.link"
        : "auth.reauth";
  const destination =
    intent === "login"
      ? new URLSearchParams(services.navigation.current().search).get(
          "return_path",
        )
      : null;
  const return_path =
    destination === "daily" ||
    destination === "friends" ||
    destination === "settings"
      ? destination
      : intent === "login"
        ? "home"
        : "settings";
  return (
    <div class="provider-option">
      <Button
        class="provider-button"
        disabled={disabled}
        onClick={() => {
          setFailed(false);
          void services.auth
            .start(
              provider,
              intent,
              locale,
              return_path,
              intent === "login" && return_path === "friends"
                ? (services.navigation.current().searchParams.get("code") ??
                    undefined)
                : undefined,
            )
            .then((result) => {
              if (result.ok) services.authEffects.redirect(result.value);
            })
            .catch(() => {
              setFailed(true);
            });
        }}
      >
        {t(key, { provider: providerName(t, provider) })}
      </Button>
      {failed && <p role="alert">{t("auth.error.auth_unavailable")}</p>}
      {!available && state.status === "ready" && (
        <small class="muted">{t("auth.disabled")}</small>
      )}
    </div>
  );
}
export function ProviderButtons({
  intent = "login",
  connectedOnly = false,
}: {
  intent?: AuthIntent;
  connectedOnly?: boolean;
}) {
  const { services } = useUi(),
    state = useSnapshot(services.auth);
  return (
    <div class="provider-list">
      {PROVIDERS.filter(
        (p) =>
          !connectedOnly || state.identities.some((i) => i.provider === p.id),
      ).map((p) => (
        <ProviderButton key={p.id} provider={p.id} intent={intent} />
      ))}
    </div>
  );
}
