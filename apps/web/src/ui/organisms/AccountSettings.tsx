import type { AuthAccount } from "@liar/protocol";
import { useState } from "preact/hooks";
import { PROVIDERS, type AuthProvider } from "../../services/auth/types";
import { useUi } from "../context";
import { useSnapshot } from "../useSnapshot";
import { Button } from "../atoms/Button";
import { Card } from "../atoms/Card";
import { Dialog } from "../atoms/Dialog";
import { AccountCard } from "./AccountCard";
import { AuthFeedback } from "./AuthFeedback";
import { NicknameForm } from "./NicknameForm";
import { ProviderButton, providerName } from "../molecules/ProviderButton";
type Confirmation =
  | { kind: "delete"; account: AuthAccount; apple: boolean }
  | { kind: "nickname"; account: AuthAccount; apple: boolean }
  | {
      kind: "unlink";
      account: AuthAccount;
      provider: AuthProvider;
      apple: boolean;
    };
export function AccountSettings() {
  const { t, services } = useUi(),
    state = useSnapshot(services.auth);
  const [confirmation, setConfirmation] = useState<Confirmation | null>(null);
  const [download, setDownload] = useState<"idle" | "ready" | "error">("idle"),
    [appleRetry, setAppleRetry] = useState(false);
  const account = state.account;
  const confirmCurrent =
    confirmation !== null &&
    account?.id === confirmation.account.id &&
    state.status === "ready" &&
    !state.working;
  const completed = async () => {
    const target = confirmation;
    if (!target || target.kind === "nickname") return;
    const result =
      target.kind === "delete"
        ? await services.auth.erase(target.account.id)
        : await services.auth.unlink(target.provider, target.account.id);
    if (result.ok) {
      setAppleRetry(target.apple && !result.value.manual_apple_disconnect);
      setConfirmation(null);
    } else if (result.code === "reauth_required") setConfirmation(null);
  };
  return (
    <section class="account-settings">
      <h2>{t("auth.account")}</h2>
      {!confirmation && <AuthFeedback />}
      {appleRetry && state.notice && (
        <p class="muted">{t("auth.appleRetry")}</p>
      )}
      {!account ? (
        <AccountCard />
      ) : (
        <>
          <Card class="account-profile">
            <strong>{account.nickname ?? t("auth.nicknameUnset")}</strong>
            <Button
              disabled={state.working}
              onClick={() =>
                setConfirmation({
                  kind: "nickname",
                  account: { ...account },
                  apple: false,
                })
              }
            >
              {t("auth.changeNickname")}
            </Button>
          </Card>
          <h3>{t("auth.providers")}</h3>
          <Card class="identity-list">
            {PROVIDERS.map((provider) => {
              const linked = state.identities.some(
                (i) => i.provider === provider.id,
              );
              return (
                <div class="identity-row" key={provider.id}>
                  <span>{providerName(t, provider.id)}</span>
                  {linked ? (
                    <Button
                      disabled={state.working || state.identities.length <= 1}
                      onClick={() =>
                        setConfirmation({
                          kind: "unlink",
                          provider: provider.id,
                          account: { ...account },
                          apple: provider.id === "apple",
                        })
                      }
                    >
                      {t("auth.disconnect", {
                        provider: providerName(t, provider.id),
                      })}
                    </Button>
                  ) : (
                    <ProviderButton provider={provider.id} intent="link" />
                  )}
                </div>
              );
            })}
          </Card>
          {state.identities.length <= 1 && (
            <p class="muted">{t("auth.lastProvider")}</p>
          )}
          <Card class="account-rights">
            <Button
              disabled={state.working}
              onClick={() => {
                const owner = account.id;
                setDownload("idle");
                void services.auth
                  .export(owner)
                  .then(async (result) => {
                    if (
                      result.ok &&
                      services.auth.read().account?.id === owner
                    ) {
                      await services.authEffects.download(result.value);
                      setDownload("ready");
                    }
                  })
                  .catch(() => setDownload("error"));
              }}
            >
              {t("auth.export")}
            </Button>
            <Button
              disabled={state.working}
              onClick={() => {
                void services.auth.logout();
              }}
            >
              {t("auth.logout")}
            </Button>
            <Button
              class="danger-action"
              disabled={state.working}
              onClick={() =>
                setConfirmation({
                  kind: "delete",
                  account: { ...account },
                  apple: state.identities.some((i) => i.provider === "apple"),
                })
              }
            >
              {t("auth.delete")}
            </Button>
          </Card>
          {download !== "idle" && (
            <output>
              {t(
                download === "ready"
                  ? "auth.exported"
                  : "auth.error.auth_unavailable",
              )}
            </output>
          )}
        </>
      )}
      <Dialog
        open={confirmation !== null}
        onClose={() => setConfirmation(null)}
        title={t(
          confirmation?.kind === "nickname"
            ? "auth.changeNickname"
            : confirmation?.kind === "unlink"
              ? "auth.providers"
              : "auth.delete",
        )}
      >
        {confirmation && (
          <>
            <AuthFeedback />
            {confirmation.kind === "nickname" ? (
              <NicknameForm
                key={confirmation.account.id}
                account={confirmation.account}
                onSaved={() => setConfirmation(null)}
              />
            ) : (
              <>
                <p>
                  {confirmation.kind === "delete"
                    ? t("auth.deleteConfirm", {
                        name:
                          confirmation.account.nickname ??
                          t("auth.nicknameUnset"),
                      })
                    : t("auth.unlinkConfirm", {
                        provider: providerName(t, confirmation.provider),
                      })}
                </p>
                <Button
                  class="danger-action"
                  disabled={!confirmCurrent}
                  onClick={() => {
                    void completed();
                  }}
                >
                  {t(
                    confirmation.kind === "delete"
                      ? "auth.confirmDelete"
                      : "auth.confirmUnlink",
                  )}
                </Button>
              </>
            )}
          </>
        )}
      </Dialog>
    </section>
  );
}
